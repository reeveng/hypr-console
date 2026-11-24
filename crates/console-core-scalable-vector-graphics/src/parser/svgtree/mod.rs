use console_core_never::Never;
use console_core_paths::StrictNormalized;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;
use std::str::FromStr;

#[rustfmt::skip]
mod names;
mod parse;
mod text;

use console_core_paths::Transform;

use crate::{
  BlendMode, ImageRendering, Opacity, ShapeRendering, SpreadMethod, TextRendering, Units,
  Visibility,
};
pub use names::{AId, EId};

pub struct Document<'input> {
  nodes: Vec<NodeData>,
  attrs: Vec<Attribute<'input>>,
  links: HashMap<String, NodeId>,
}

impl<'input> Document<'input> {
  #[inline]
  pub fn root<'a>(
    &'a self,
  ) -> core::result::Result<SvgNode<'a, 'input>, console_core_never::Never> {
    self.get(NodeId::ROOT)
  }

  #[inline]
  pub fn root_element<'a>(
    &'a self,
  ) -> Result<SvgNode<'a, 'input>, console_core_extensible_markup::Error> {
    let Ok(root) = self.root();
    let Ok(first_element_child) = root.first_element_child();

    first_element_child.ok_or(console_core_extensible_markup::Error::NoRootNode)
  }

  #[inline]
  pub fn descendants<'a>(
    &'a self,
  ) -> core::result::Result<Descendants<'a, 'input>, console_core_never::Never> {
    let Ok(root) = self.root();
    let Ok(descendants) = root.descendants();

    Ok(descendants)
  }

  #[inline]
  pub fn element_by_id<'a>(
    &'a self,
    id: &str,
  ) -> core::result::Result<Option<SvgNode<'a, 'input>>, console_core_never::Never> {
    let node_id = match self.links.get(id) {
      Some(value) => value,
      None => return Ok(None),
    };

    Ok(Some({
      let Ok(explicit_value) = self.get(*node_id);
      explicit_value
    }))
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit014_no_index_slice",
    expect(
      explicit014_no_index_slice,
      reason = "Node IDs come from the initial root slot or append, and the document never removes or renumbers a slot"
    )
  )]
  fn get<'a>(
    &'a self,
    id: NodeId,
  ) -> core::result::Result<SvgNode<'a, 'input>, console_core_never::Never> {
    let Ok(index) = id.get();

    Ok(SvgNode {
      id,
      d: &self.nodes[match ::console_core_number_conversion::index(index) {
        ::core::result::Result::Ok(native_position) => native_position,
      }],
      doc: self,
    })
  }
}

impl std::fmt::Debug for Document<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let Ok(root) = self.root();

    match root.has_children() {
      Ok(crate::Presence::Absent) => return write!(f, "Document []"),
      Ok(crate::Presence::Present) => {}
    }

    writeln!(f, "Document [")?;
    let Ok(events) = root.traverse();
    let mut depth = 1u16;

    for event in events.skip(1) {
      match event {
        Edge::Open(node) => {
          let next_depth = write_open_node(node, f, depth)?;
          depth = next_depth;
        }
        Edge::Close(node) => {
          match (node == root).then_some(()) {
            Some(()) => break,
            None => {}
          }

          let checked_value = write_close_element(node, f, depth)?;
          depth = checked_value;
        }
      }
    }

    writeln!(f, "]")
  }
}

fn write_open_node(
  node: SvgNode,
  f: &mut std::fmt::Formatter,
  depth: u16,
) -> Result<u16, std::fmt::Error> {
  match node.is_element() {
    Ok(crate::NodeMatch::Different) => {
      write_debug_line(f, depth, format_args!("{node:?}"))?;

      Ok(depth)
    }
    Ok(crate::NodeMatch::Matches) => {
      write_open_element(node, f, depth)?;

      match node.has_children() {
        Ok(crate::Presence::Present) => Ok(depth.saturating_add(2)),
        Ok(crate::Presence::Absent) => Ok(depth),
      }
    }
  }
}

fn write_close_element(
  node: SvgNode,
  f: &mut std::fmt::Formatter,
  depth: u16,
) -> Result<u16, std::fmt::Error> {
  match node.is_element() {
    Ok(crate::NodeMatch::Different) => return Ok(depth),
    Ok(crate::NodeMatch::Matches) => {}
  }

  let child_depth = match node.has_children() {
    Ok(crate::Presence::Present) => {
      let indentation = depth.saturating_sub(2);
      write_debug_line(f, indentation, format_args!("    ]"))?;

      indentation
    }
    Ok(crate::Presence::Absent) => depth,
  };
  write_debug_line(f, child_depth, format_args!("}}"))?;

  Ok(child_depth)
}

fn write_debug_line(
  f: &mut std::fmt::Formatter,
  depth: u16,
  text: std::fmt::Arguments,
) -> std::fmt::Result {
  for _ in 0..depth {
    write!(f, "    ")?;
  }

  writeln!(f, "{text}")
}

fn write_open_element(
  node: SvgNode,
  f: &mut std::fmt::Formatter,
  depth: u16,
) -> std::fmt::Result {
  let Ok(tag) = node.tag_name();
  let Ok(attributes) = node.attributes();
  write_debug_line(f, depth, format_args!("Element {{"))?;
  write_debug_line(f, depth, format_args!("    tag_name: {tag:?}"))?;

  match attributes {
    [] => {}
    [_, ..] => {
      let attr_depth = depth.saturating_add(1);
      write_debug_line(f, attr_depth, format_args!("attributes: ["))?;

      for attribute in attributes {
        write_debug_line(
          f,
          attr_depth.saturating_add(1),
          format_args!("{attribute:?}"),
        )?;
      }

      write_debug_line(f, attr_depth, format_args!("]"))?;
    }
  }

  match node.has_children() {
    Ok(crate::Presence::Present) => write_debug_line(f, depth, format_args!("    children: [")),
    Ok(crate::Presence::Absent) => Ok(()),
  }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShortRange {
  start: u32,
  end: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct NodeId(NonZeroU32);

impl NodeId {
  const ROOT: Self = Self(NonZeroU32::MIN);

  fn for_new_slot(index: u64) -> Result<Self, console_core_extensible_markup::Error> {
    let index_2 = u32::try_from(index)
      .map_err(|_overflow| console_core_extensible_markup::Error::NodesLimitReached)?;
    let slot = match index_2.checked_add(1).and_then(NonZeroU32::new) {
      Some(slot) => slot,
      None => return Err(console_core_extensible_markup::Error::NodesLimitReached),
    };

    Ok(Self(slot))
  }

  #[inline]
  fn get(self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(self.0.get().saturating_sub(1))
  }
}

pub(crate) enum NodeKind {
  Root,
  Element {
    tag_name: EId,
    attributes: ShortRange,
  },
  Text(String),
}

struct NodeData {
  parent: Option<NodeId>,
  next_sibling: Option<NodeId>,
  children: Option<(NodeId, NodeId)>,
  kind: NodeKind,
}

#[derive(Clone, Copy, Debug)]
enum DeclarationImportance {
  Normal,
  Important,
}

impl From<bool> for DeclarationImportance {
  fn from(important: bool) -> Self {
    match important.then_some(()) {
      Some(()) => Self::Important,
      None => Self::Normal,
    }
  }
}

#[derive(Clone)]
pub struct Attribute<'input> {
  pub name: AId,

  pub value: console_core_extensible_markup::StringStorage<'input>,

  importance: DeclarationImportance,
}

impl std::fmt::Debug for Attribute<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
    write!(
      f,
      "Attribute {{ name: {:?}, value: {}, important: {} }}",
      self.name,
      self.value,
      matches!(self.importance, DeclarationImportance::Important)
    )
  }
}

#[derive(Clone, Copy)]
pub struct SvgNode<'a, 'input: 'a> {
  id: NodeId,
  doc: &'a Document<'input>,
  d: &'a NodeData,
}

impl Eq for SvgNode<'_, '_> {}

impl PartialEq for SvgNode<'_, '_> {
  #[inline]
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && std::ptr::eq(self.doc, other.doc) && std::ptr::eq(self.d, other.d)
  }
}

impl<'a, 'input: 'a> SvgNode<'a, 'input> {
  #[inline]
  fn id(&self) -> core::result::Result<NodeId, console_core_never::Never> {
    Ok(self.id)
  }

  #[inline]
  pub fn is_element(&self) -> core::result::Result<crate::NodeMatch, console_core_never::Never> {
    Ok(match self.d.kind {
      NodeKind::Element { .. } => crate::NodeMatch::Matches,
      NodeKind::Root | NodeKind::Text(_) => crate::NodeMatch::Different,
    })
  }

  #[inline]
  pub fn is_text(&self) -> core::result::Result<crate::NodeMatch, console_core_never::Never> {
    Ok(match self.d.kind {
      NodeKind::Text(_) => crate::NodeMatch::Matches,
      NodeKind::Root | NodeKind::Element { .. } => crate::NodeMatch::Different,
    })
  }

  #[inline]
  pub fn document(
    &self,
  ) -> core::result::Result<&'a Document<'input>, console_core_never::Never> {
    Ok(self.doc)
  }

  #[inline]
  pub fn tag_name(&self) -> core::result::Result<Option<EId>, console_core_never::Never> {
    Ok(match self.d.kind {
      NodeKind::Element { tag_name, .. } => Some(tag_name),
      NodeKind::Root | NodeKind::Text(_) => None,
    })
  }

  #[inline]
  pub fn element_id(&self) -> core::result::Result<&'a str, console_core_never::Never> {
    let Ok(attribute) = self.attribute(AId::Id);

    Ok(match attribute {
      Some(id) => id,
      None => "",
    })
  }

  pub fn attribute<T: FromValue<'a, 'input>>(
    &self,
    aid: AId,
  ) -> core::result::Result<Option<T>, console_core_never::Never> {
    let Ok(attributes) = self.attributes();
    let value = match attributes.iter().find(|a| a.name == aid).map(|a| {
      let Ok(explicit_value) = a.value.as_str();
      explicit_value
    }) {
      Some(value) => value,
      None => return Ok(None),
    };

    let is_possible_none = matches!(
      aid,
      AId::Mask
        | AId::MarkerStart
        | AId::MarkerMid
        | AId::MarkerEnd
        | AId::ClipPath
        | AId::Filter
        | AId::FontSizeAdjust
        | AId::TextDecoration
        | AId::Stroke
        | AId::StrokeDasharray
    );

    match ::core::primitive::bool::then_some(is_possible_none && value == "none", ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    Ok(match T::parse(*self, aid, value) {
      Some(v) => Some(v),
      None => {
        console_core_logging::warn!("Failed to parse {} value: '{}'.", aid, value);
        None
      }
    })
  }

  pub fn try_attribute<T: FromValue<'a, 'input>>(
    &self,
    aid: AId,
  ) -> core::result::Result<Option<T>, console_core_never::Never> {
    let Ok(attributes) = self.attributes();
    let value = match attributes.iter().find(|a| a.name == aid).map(|a| {
      let Ok(explicit_value) = a.value.as_str();
      explicit_value
    }) {
      Some(value) => value,
      None => return Ok(None),
    };

    Ok(T::parse(*self, aid, value))
  }

  #[inline]
  fn node_attribute(
    &self,
    aid: AId,
  ) -> core::result::Result<Option<SvgNode<'a, 'input>>, console_core_never::Never> {
    let Ok(attribute) = self.attribute(aid);
    let value = match attribute {
      Some(value) => value,
      None => return Ok(None),
    };
    Ok(Self::parse(*self, aid, value))
  }

  #[inline]
  pub fn has_attribute(
    &self,
    aid: AId,
  ) -> core::result::Result<crate::Presence, console_core_never::Never> {
    let Ok(attributes) = self.attributes();

    Ok(
      match ::core::primitive::bool::then_some(attributes.iter().any(|a| a.name == aid), ()) {
        ::core::option::Option::Some(()) => crate::Presence::Present,
        ::core::option::Option::None => crate::Presence::Absent,
      },
    )
  }

  #[inline]
  pub fn attributes(
    &self,
  ) -> core::result::Result<&'a [Attribute<'input>], console_core_never::Never> {
    Ok(match self.d.kind {
      NodeKind::Element { ref attributes, .. } => {
        let Ok(start) = console_core_number_conversion::index(attributes.start);
        let Ok(end) = console_core_number_conversion::index(attributes.end);

        match self.doc.attrs.get(start..end) {
          Some(attributes_2) => attributes_2,
          None => &[],
        }
      }
      NodeKind::Root | NodeKind::Text(_) => &[],
    })
  }

  #[inline]
  fn attribute_id(
    &self,
    aid: AId,
  ) -> core::result::Result<Option<u64>, console_core_never::Never> {
    Ok(match self.d.kind {
      NodeKind::Element { attributes, .. } => {
        let Ok(own_attributes) = self.attributes();

        own_attributes
          .iter()
          .position(|attribute| attribute.name == aid)
          .and_then(|offset| {
            let Ok(offset_2) = console_core_number_conversion::fixed_index(offset);
            u64::from(attributes.start).checked_add(offset_2)
          })
      }
      NodeKind::Root | NodeKind::Text(_) => None,
    })
  }

  pub fn find_attribute<T: FromValue<'a, 'input>>(
    &self,
    aid: AId,
  ) -> core::result::Result<Option<T>, console_core_never::Never> {
    let Ok(find_attribute_impl) = self.find_attribute_impl(aid);

    Ok({
      let Ok(explicit_value) = match find_attribute_impl {
        Some(value) => value,
        None => return Ok(None),
      }
      .attribute(aid);
      explicit_value
    })
  }

  fn find_attribute_impl(
    &self,
    aid: AId,
  ) -> core::result::Result<Option<SvgNode<'a, 'input>>, console_core_never::Never> {
    let Ok(is_inheritable) = aid.is_inheritable();

    Ok(match is_inheritable {
      crate::IsInheritableAnswer::Yes => {
        let Ok(ancestors) = self.ancestors();

        for n in ancestors {
          let Ok(has_attribute) = n.has_attribute(aid);

          match has_attribute {
            crate::Presence::Present => {
              return Ok(Some(n));
            }
            crate::Presence::Absent => {}
          }
        }

        None
      }
      crate::IsInheritableAnswer::No => {
        let Ok(has_attribute) = self.has_attribute(aid);

        match has_attribute {
          crate::Presence::Present => Some(*self),
          crate::Presence::Absent => {
            let Ok(parent_element) = self.parent_element();
            let n = match parent_element {
              Some(value) => value,
              None => return Ok(None),
            };

            let Ok(n_has_attribute) = n.has_attribute(aid);

            match n_has_attribute {
              crate::Presence::Present => Some(n),
              crate::Presence::Absent => None,
            }
          }
        }
      }
    })
  }

  #[inline]
  pub fn text(&self) -> core::result::Result<&'a str, console_core_never::Never> {
    Ok(match self.d.kind {
      NodeKind::Element { .. } => {
        let Ok(first_child) = self.first_child();

        match first_child.map(|child| &child.d.kind) {
          Some(NodeKind::Text(text)) => text,
          Some(NodeKind::Root | NodeKind::Element { .. }) | None => "",
        }
      }
      NodeKind::Text(ref text) => text,
      NodeKind::Root => "",
    })
  }

  #[inline]
  pub fn parent(&self) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(self.d.parent.map(|id| {
      let Ok(doc) = self.doc.get(id);

      doc
    }))
  }

  #[inline]
  pub fn parent_element(&self) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(ancestors) = self.ancestors();

    Ok(ancestors.skip(1).find(|n| {
      let Ok(is_element) = n.is_element();

      matches!(is_element, crate::NodeMatch::Matches)
    }))
  }

  #[inline]
  pub fn next_sibling(&self) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(self.d.next_sibling.map(|id| {
      let Ok(doc) = self.doc.get(id);

      doc
    }))
  }

  #[inline]
  pub fn first_child(&self) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(self.d.children.map(|(id, _)| {
      let Ok(doc) = self.doc.get(id);

      doc
    }))
  }

  #[inline]
  pub fn first_element_child(
    &self,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(mut children) = self.children();

    Ok(children.find(|n| {
      let Ok(is_element) = n.is_element();

      matches!(is_element, crate::NodeMatch::Matches)
    }))
  }

  #[inline]
  pub fn last_child(&self) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(self.d.children.map(|(_, id)| {
      let Ok(doc) = self.doc.get(id);

      doc
    }))
  }

  #[inline]
  pub fn has_children(&self) -> core::result::Result<crate::Presence, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.d.children.is_some(), ()) {
        ::core::option::Option::Some(()) => crate::Presence::Present,
        ::core::option::Option::None => crate::Presence::Absent,
      },
    )
  }

  #[inline]
  pub fn ancestors(
    &self,
  ) -> core::result::Result<Ancestors<'a, 'input>, console_core_never::Never> {
    Ok(Ancestors(Some(*self)))
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
  fn traverse(&self) -> core::result::Result<Traverse<'a, 'input>, console_core_never::Never> {
    Ok(Traverse {
      root: *self,
      edge: None,
    })
  }

  #[inline]
  pub fn descendants(
    &self,
  ) -> core::result::Result<Descendants<'a, 'input>, console_core_never::Never> {
    let Ok(traverse) = self.traverse();

    Ok(Descendants(traverse))
  }

  #[inline]
  pub fn href_iter(
    &self,
  ) -> core::result::Result<HrefIter<'a, 'input>, console_core_never::Never> {
    let Ok(doc) = self.document();
    let Ok(origin) = self.id();

    Ok(HrefIter {
      doc,
      origin,
      state: HrefVisit::First,
      visited: HashSet::new(),
    })
  }
}

impl std::fmt::Debug for SvgNode<'_, '_> {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
    match self.d.kind {
      NodeKind::Root => write!(f, "Root"),
      NodeKind::Element { .. } => {
        let Ok(tag_name) = self.tag_name();
        let Ok(attributes) = self.attributes();

        write!(
          f,
          "Element {{ tag_name: {:?}, attributes: {:?} }}",
          tag_name, attributes
        )
      }
      NodeKind::Text(ref text) => write!(f, "Text({:?})", text),
    }
  }
}

#[derive(Clone, Debug)]
pub struct Ancestors<'a, 'input: 'a>(Option<SvgNode<'a, 'input>>);

impl<'a, 'input: 'a> Iterator for Ancestors<'a, 'input> {
  type Item = SvgNode<'a, 'input>;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    let node = self.0.take();
    self.0 = node.as_ref().and_then(|argument_0| {
      let Ok(parent) = SvgNode::parent(argument_0);

      parent
    });
    node
  }
}

#[derive(Clone, Debug)]
pub struct Children<'a, 'input: 'a> {
  front: Option<SvgNode<'a, 'input>>,
  back: Option<SvgNode<'a, 'input>>,
}

impl<'a, 'input: 'a> Iterator for Children<'a, 'input> {
  type Item = SvgNode<'a, 'input>;

  fn next(&mut self) -> Option<Self::Item> {
    let node = self.front.take();

    match ::core::primitive::bool::then_some(self.front == self.back, ()) {
      ::core::option::Option::Some(()) => {
        self.back = None;
      }
      ::core::option::Option::None => {
        self.front = node.as_ref().and_then(|argument_0| {
          let Ok(sibling) = SvgNode::next_sibling(argument_0);

          sibling
        });
      }
    }

    node
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Edge<'a, 'input: 'a> {
  Open(SvgNode<'a, 'input>),
  Close(SvgNode<'a, 'input>),
}

#[derive(Clone, Debug)]
struct Traverse<'a, 'input: 'a> {
  root: SvgNode<'a, 'input>,
  edge: Option<Edge<'a, 'input>>,
}

impl<'a, 'input: 'a> Iterator for Traverse<'a, 'input> {
  type Item = Edge<'a, 'input>;

  fn next(&mut self) -> Option<Self::Item> {
    self.edge = match self.edge {
      Some(Edge::Open(node)) => {
        let Ok(child) = node.first_child();
        Some(match child {
          Some(first_child) => Edge::Open(first_child),
          None => Edge::Close(node),
        })
      }
      Some(Edge::Close(node)) => match (node == self.root).then_some(()) {
        Some(()) => None,
        None => {
          let Ok(sibling) = node.next_sibling();

          match sibling {
            Some(next_sibling) => Some(Edge::Open(next_sibling)),
            None => {
              let Ok(parent) = node.parent();
              parent.map(Edge::Close)
            }
          }
        }
      },
      None => Some(Edge::Open(self.root)),
    };

    self.edge
  }
}

#[derive(Clone, Debug)]
pub struct Descendants<'a, 'input: 'a>(Traverse<'a, 'input>);

impl<'a, 'input: 'a> Iterator for Descendants<'a, 'input> {
  type Item = SvgNode<'a, 'input>;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    for edge in &mut self.0 {
      match edge {
        Edge::Open(node) => {
          return Some(node);
        }
        Edge::Close(_) => {}
      }
    }

    None
  }
}

#[derive(Clone, Copy, Debug)]
enum HrefVisit {
  First,
  ContinueFrom(NodeId),
  Finished,
}

#[derive(Clone, Debug)]
pub struct HrefIter<'a, 'input: 'a> {
  doc: &'a Document<'input>,
  origin: NodeId,
  state: HrefVisit,
  visited: HashSet<NodeId>,
}

impl<'a, 'input: 'a> Iterator for HrefIter<'a, 'input> {
  type Item = SvgNode<'a, 'input>;

  fn next(&mut self) -> Option<Self::Item> {
    let current = match self.state {
      HrefVisit::First => {
        self.state = HrefVisit::ContinueFrom(self.origin);
        let Ok(origin) = self.doc.get(self.origin);

        return Some(origin);
      }
      HrefVisit::ContinueFrom(current) => current,
      HrefVisit::Finished => return None,
    };
    let Ok(node) = self.doc.get(current);
    let Ok(reference) = node.node_attribute(AId::Href);
    let link = match reference {
      Some(link) => link,
      None => {
        self.state = HrefVisit::Finished;

        return None;
      }
    };
    let Ok(link_id) = link.id();

    match ::core::primitive::bool::then_some(
      link_id != self.origin && link_id != current && self.visited.insert(link_id),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        self.state = HrefVisit::ContinueFrom(link_id);

        Some(link)
      }
      ::core::option::Option::None => {
        let Ok(origin) = self.doc.get(self.origin);
        let Ok(element_id) = origin.element_id();
        console_core_logging::warn!("Stopping cyclic SVG href chain from #{element_id}");
        self.state = HrefVisit::Finished;

        None
      }
    }
  }
}

impl EId {
  pub fn is_graphic(
    &self,
  ) -> core::result::Result<crate::IsGraphicAnswer, console_core_never::Never> {
    Ok(crate::IsGraphicAnswer::from(matches!(
      self,
      EId::Circle
        | EId::Ellipse
        | EId::Image
        | EId::Line
        | EId::Path
        | EId::Polygon
        | EId::Polyline
        | EId::Rectangle
        | EId::Text
        | EId::Use
    )))
  }

  pub fn is_paint_server(
    &self,
  ) -> core::result::Result<crate::IsPaintServerAnswer, console_core_never::Never> {
    Ok(crate::IsPaintServerAnswer::from(matches!(
      self,
      EId::LinearGradient | EId::RadialGradient | EId::Pattern
    )))
  }
}

impl AId {
  #[cfg_attr(
    dylint_lib = "explicit050_no_long_function",
    expect(explicit050_no_long_function, reason = "the presentation property list is one membership test with no separate stages or decisions")
  )]
  fn is_presentation(&self) -> Result<crate::IsPresentationAnswer, console_core_never::Never> {
    Ok(crate::IsPresentationAnswer::from(matches!(
      self,
      AId::AlignmentBaseline
        | AId::BaselineShift
        | AId::BackgroundColor
        | AId::ClipPath
        | AId::ClipRule
        | AId::Color
        | AId::ColorInterpolation
        | AId::ColorInterpolationFilters
        | AId::ColorRendering
        | AId::Direction
        | AId::Display
        | AId::DominantBaseline
        | AId::Fill
        | AId::FillOpacity
        | AId::FillRule
        | AId::Filter
        | AId::FloodColor
        | AId::FloodOpacity
        | AId::FontFamily
        | AId::FontKerning
        | AId::FontOpticalSizing
        | AId::FontSize
        | AId::FontSizeAdjust
        | AId::FontStretch
        | AId::FontStyle
        | AId::FontVariant
        | AId::FontWeight
        | AId::FontVariationSettings
        | AId::GlyphOrientationHorizontal
        | AId::GlyphOrientationVertical
        | AId::ImageRendering
        | AId::Isolation
        | AId::LetterSpacing
        | AId::LightingColor
        | AId::MarkerEnd
        | AId::MarkerMid
        | AId::MarkerStart
        | AId::Mask
        | AId::MaskType
        | AId::MixBlendMode
        | AId::Opacity
        | AId::Overflow
        | AId::PaintOrder
        | AId::ShapeRendering
        | AId::StopColor
        | AId::StopOpacity
        | AId::Stroke
        | AId::StrokeDasharray
        | AId::StrokeDashoffset
        | AId::StrokeLinecap
        | AId::StrokeLinejoin
        | AId::StrokeMiterlimit
        | AId::StrokeOpacity
        | AId::StrokeWidth
        | AId::TextAnchor
        | AId::TextDecoration
        | AId::TextOverflow
        | AId::TextRendering
        | AId::Transform
        | AId::TransformOrigin
        | AId::UnicodeBidi
        | AId::VectorEffect
        | AId::Visibility
        | AId::WhiteSpace
        | AId::WordSpacing
        | AId::WritingMode
    )))
  }

  fn is_inheritable(
    &self,
  ) -> core::result::Result<crate::IsInheritableAnswer, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        {
          let Ok(is_presentation) = self.is_presentation();

          match is_presentation {
            crate::IsPresentationAnswer::Yes => !matches!(
              {
                let Ok(explicit_value) = is_non_inheritable(*self);
                explicit_value
              },
              crate::IsNonInheritableAnswer::Yes
            ),
            crate::IsPresentationAnswer::No => false,
          }
        },
        (),
      ) {
        ::core::option::Option::Some(()) => crate::IsInheritableAnswer::Yes,
        ::core::option::Option::None => crate::IsInheritableAnswer::No,
      },
    )
  }

  fn allows_inherit_value(
    &self,
  ) -> core::result::Result<crate::AllowsInheritValueAnswer, console_core_never::Never> {
    Ok(crate::AllowsInheritValueAnswer::from(matches!(
      self,
      AId::AlignmentBaseline
        | AId::BaselineShift
        | AId::ClipPath
        | AId::ClipRule
        | AId::Color
        | AId::ColorInterpolationFilters
        | AId::Direction
        | AId::Display
        | AId::DominantBaseline
        | AId::Fill
        | AId::FillOpacity
        | AId::FillRule
        | AId::Filter
        | AId::FloodColor
        | AId::FloodOpacity
        | AId::FontFamily
        | AId::FontKerning
        | AId::FontOpticalSizing
        | AId::FontSize
        | AId::FontStretch
        | AId::FontStyle
        | AId::FontVariant
        | AId::FontWeight
        | AId::ImageRendering
        | AId::Kerning
        | AId::LetterSpacing
        | AId::MarkerEnd
        | AId::MarkerMid
        | AId::MarkerStart
        | AId::Mask
        | AId::Opacity
        | AId::Overflow
        | AId::ShapeRendering
        | AId::StopColor
        | AId::StopOpacity
        | AId::Stroke
        | AId::StrokeDasharray
        | AId::StrokeDashoffset
        | AId::StrokeLinecap
        | AId::StrokeLinejoin
        | AId::StrokeMiterlimit
        | AId::StrokeOpacity
        | AId::StrokeWidth
        | AId::TextAnchor
        | AId::TextDecoration
        | AId::TextRendering
        | AId::Visibility
        | AId::WordSpacing
        | AId::WritingMode
    )))
  }
}

fn is_non_inheritable(
  id: AId,
) -> core::result::Result<crate::IsNonInheritableAnswer, console_core_never::Never> {
  Ok(crate::IsNonInheritableAnswer::from(matches!(
    id,
    AId::AlignmentBaseline
      | AId::BaselineShift
      | AId::ClipPath
      | AId::Display
      | AId::DominantBaseline
      | AId::Filter
      | AId::FloodColor
      | AId::FloodOpacity
      | AId::Mask
      | AId::Opacity
      | AId::Overflow
      | AId::LightingColor
      | AId::StopColor
      | AId::StopOpacity
      | AId::TextDecoration
      | AId::Transform
      | AId::TransformOrigin
  )))
}

fn recover_attribute<T, E: std::fmt::Debug>(
  parsed: Result<T, E>,
  name: AId,
  source: &str,
) -> Result<Option<T>, Never> {
  Ok(match parsed {
    Ok(value) => Some(value),
    Err(problem) => {
      console_core_logging::debug!("Ignoring malformed SVG {name} value {source:?}: {problem:?}");

      None
    }
  })
}

pub trait FromValue<'a, 'input: 'a>: Sized {
  fn parse(node: SvgNode<'a, 'input>, aid: AId, value: &'a str) -> Option<Self>;
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for &'a str {
  fn parse(_: SvgNode<'a, 'input>, _: AId, value: &'a str) -> Option<Self> {
    Some(value)
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for f32 {
  fn parse(_: SvgNode, aid: AId, value: &str) -> Option<Self> {
    use console_core_scalable_vector_graphics_values::Number;

    let Ok(parsed) = recover_attribute(Number::from_str(value), aid, value);
    let number = parsed?;
    let Ok(value_2) = console_core_number_conversion::nearest_f32_from_f64(number.0);

    Some(value_2)
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input>
  for console_core_scalable_vector_graphics_values::Length
{
  fn parse(_: SvgNode, aid: AId, value: &str) -> Option<Self> {
    let Ok(length) = recover_attribute(Self::from_str(value), aid, value);

    length
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for Opacity {
  fn parse(_: SvgNode, aid: AId, value: &str) -> Option<Self> {
    use console_core_scalable_vector_graphics_values::{Length, LengthUnit};

    let Ok(parsed) = recover_attribute(Length::from_str(value), aid, value);
    let length = parsed?;
    let Ok(number) = console_core_number_conversion::nearest_f32_from_f64(length.number);

    match length.unit {
      LengthUnit::Percent => Some(Opacity::new_clamped(number / 100.0)),
      LengthUnit::None => Some(Opacity::new_clamped(number)),
      LengthUnit::Em
      | LengthUnit::Ex
      | LengthUnit::Px
      | LengthUnit::Inches
      | LengthUnit::Cm
      | LengthUnit::Mm
      | LengthUnit::Pt
      | LengthUnit::Pc => None,
    }
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for Transform {
  fn parse(_: SvgNode, aid: AId, value: &str) -> Option<Self> {
    let Ok(parsed) = recover_attribute(
      console_core_scalable_vector_graphics_values::Transform::from_str(value),
      aid,
      value,
    );
    let ts = parsed?;

    let Ok(ts_2) = Transform::from_row(
      match ::console_core_number_conversion::nearest_f32_from_f64(ts.a) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      },
      match ::console_core_number_conversion::nearest_f32_from_f64(ts.b) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      },
      match ::console_core_number_conversion::nearest_f32_from_f64(ts.c) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      },
      match ::console_core_number_conversion::nearest_f32_from_f64(ts.d) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      },
      match ::console_core_number_conversion::nearest_f32_from_f64(ts.e) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      },
      match ::console_core_number_conversion::nearest_f32_from_f64(ts.f) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      },
    );

    let Ok(is_valid) = ts_2.is_valid();

    match is_valid {
      console_core_paths::IsValidAnswer::Yes => Some(ts_2),
      console_core_paths::IsValidAnswer::No => Some(Transform::default()),
    }
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input>
  for console_core_scalable_vector_graphics_values::TransformOrigin
{
  fn parse(_: SvgNode, aid: AId, value: &str) -> Option<Self> {
    let Ok(parsed) = recover_attribute(Self::from_str(value), aid, value);

    parsed
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input>
  for console_core_scalable_vector_graphics_values::ViewBox
{
  fn parse(_: SvgNode, aid: AId, value: &str) -> Option<Self> {
    let Ok(parsed) = recover_attribute(Self::from_str(value), aid, value);

    parsed
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for Units {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "userSpaceOnUse" => Some(Units::UserSpaceOnUse),
      "objectBoundingBox" => Some(Units::ObjectBoundingBox),
      _ => None,
    }
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input>
  for console_core_scalable_vector_graphics_values::AspectRatio
{
  fn parse(_: SvgNode, aid: AId, value: &str) -> Option<Self> {
    let Ok(parsed) = recover_attribute(Self::from_str(value), aid, value);

    parsed
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input>
  for console_core_scalable_vector_graphics_values::PaintOrder
{
  fn parse(_: SvgNode, aid: AId, value: &str) -> Option<Self> {
    let Ok(parsed) = recover_attribute(Self::from_str(value), aid, value);

    parsed
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input>
  for console_core_scalable_vector_graphics_values::Color
{
  fn parse(_: SvgNode, aid: AId, value: &str) -> Option<Self> {
    let Ok(parsed) = recover_attribute(Self::from_str(value), aid, value);

    parsed
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input>
  for console_core_scalable_vector_graphics_values::Angle
{
  fn parse(_: SvgNode, aid: AId, value: &str) -> Option<Self> {
    let Ok(parsed) = recover_attribute(Self::from_str(value), aid, value);

    parsed
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input>
  for console_core_scalable_vector_graphics_values::EnableBackground
{
  fn parse(_: SvgNode, aid: AId, value: &str) -> Option<Self> {
    let Ok(parsed) = recover_attribute(Self::from_str(value), aid, value);

    parsed
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input>
  for console_core_scalable_vector_graphics_values::Paint<'a>
{
  fn parse(_: SvgNode, aid: AId, value: &'a str) -> Option<Self> {
    let Ok(parsed) = recover_attribute(Self::from_str(value), aid, value);

    parsed
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for Vec<f32> {
  fn parse(_: SvgNode, aid: AId, value: &str) -> Option<Self> {
    use console_core_scalable_vector_graphics_values::NumberListParser;

    let mut list = Vec::new();

    for item in NumberListParser::from(value) {
      let Ok(parsed) = recover_attribute(item, aid, value);
      let number = parsed?;
      let Ok(floating) = console_core_number_conversion::nearest_f32_from_f64(number);
      list.push(floating);
    }

    Some(list)
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input>
  for Vec<console_core_scalable_vector_graphics_values::Length>
{
  fn parse(_: SvgNode, aid: AId, value: &str) -> Option<Self> {
    use console_core_scalable_vector_graphics_values::LengthListParser;

    let mut list = Vec::new();

    for item in LengthListParser::from(value) {
      let Ok(parsed) = recover_attribute(item, aid, value);
      let length = parsed?;
      list.push(length);
    }

    Some(list)
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for Visibility {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "visible" => Some(Visibility::Visible),
      "hidden" => Some(Visibility::Hidden),
      "collapse" => Some(Visibility::Collapse),
      _ => None,
    }
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for SpreadMethod {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "pad" => Some(SpreadMethod::Pad),
      "reflect" => Some(SpreadMethod::Reflect),
      "repeat" => Some(SpreadMethod::Repeat),
      _ => None,
    }
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for ShapeRendering {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "optimizeSpeed" => Some(ShapeRendering::OptimizeSpeed),
      "crispEdges" => Some(ShapeRendering::CrispEdges),
      "auto" | "geometricPrecision" => Some(ShapeRendering::GeometricPrecision),
      _ => None,
    }
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for TextRendering {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "optimizeSpeed" => Some(TextRendering::OptimizeSpeed),
      "auto" | "optimizeLegibility" => Some(TextRendering::OptimizeLegibility),
      "geometricPrecision" => Some(TextRendering::GeometricPrecision),
      _ => None,
    }
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for ImageRendering {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "auto" | "optimizeQuality" => Some(ImageRendering::OptimizeQuality),
      "optimizeSpeed" => Some(ImageRendering::OptimizeSpeed),
      "smooth" => Some(ImageRendering::Smooth),
      "high-quality" => Some(ImageRendering::HighQuality),
      "crisp-edges" => Some(ImageRendering::CrispEdges),
      "pixelated" => Some(ImageRendering::Pixelated),
      _ => None,
    }
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for BlendMode {
  fn parse(_: SvgNode, _: AId, value: &str) -> Option<Self> {
    match value {
      "normal" => Some(BlendMode::Normal),
      "multiply" => Some(BlendMode::Multiply),
      "screen" => Some(BlendMode::Screen),
      "overlay" => Some(BlendMode::Overlay),
      "darken" => Some(BlendMode::Darken),
      "lighten" => Some(BlendMode::Lighten),
      "color-dodge" => Some(BlendMode::ColorDodge),
      "color-burn" => Some(BlendMode::ColorBurn),
      "hard-light" => Some(BlendMode::HardLight),
      "soft-light" => Some(BlendMode::SoftLight),
      "difference" => Some(BlendMode::Difference),
      "exclusion" => Some(BlendMode::Exclusion),
      "hue" => Some(BlendMode::Hue),
      "saturation" => Some(BlendMode::Saturation),
      "color" => Some(BlendMode::Color),
      "luminosity" => Some(BlendMode::Luminosity),
      _ => None,
    }
  }
}

impl<'a, 'input: 'a> FromValue<'a, 'input> for SvgNode<'a, 'input> {
  fn parse(node: SvgNode<'a, 'input>, aid: AId, value: &str) -> Option<Self> {
    use console_core_scalable_vector_graphics_values::{FuncIRI, IRI};

    let Ok(reference) = match ::core::primitive::bool::then_some(aid == AId::Href, ()) {
      ::core::option::Option::Some(()) => {
        recover_attribute(IRI::from_str(value).map(|iri| iri.0), aid, value)
      }
      ::core::option::Option::None => {
        recover_attribute(FuncIRI::from_str(value).map(|iri| iri.0), aid, value)
      }
    };
    let id = reference?;

    let Ok(document) = node.document();
    let Ok(element_by_id) = document.element_by_id(id);

    element_by_id
  }
}
