use console_core_never::Never;
use console_core_extensible_markup::{Children as XmlChildren, Error};

use super::{AId, Document, EId, NodeId, NodeKind, SvgNode};

const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

pub(crate) fn parse_svg_text_element<'input>(
  parent: console_core_extensible_markup::Node<'_, 'input>,
  parent_id: NodeId,
  style_sheet: &console_core_cascading_style_sheets::StyleSheet,
  doc: &mut Document<'input>,
) -> Result<(), Error> {
  let Ok(has_attribute) = ({
    let Ok(explicit_value) = doc.get(parent_id);
    explicit_value
  })
  .has_attribute(AId::Space);
  let space = match has_attribute {
    crate::Presence::Present => {
      let Ok(xmlspace) = get_xmlspace(doc, parent_id, XmlSpace::Default);

      xmlspace
    }
    crate::Presence::Absent => {
      let Ok(mut ancestors) = ({
        let Ok(explicit_value) = doc.get(parent_id);
        explicit_value
      })
      .ancestors();

      match ancestors.find(|n| {
        let Ok(n_has_attribute) = n.has_attribute(AId::Space);

        matches!(n_has_attribute, crate::Presence::Present)
      }) {
        Some(node) => {
          let Ok(xmlspace) = get_xmlspace(doc, node.id, XmlSpace::Default);

          xmlspace
        }
        None => XmlSpace::Default,
      }
    }
  };

  parse_svg_text_element_impl(parent, parent_id, style_sheet, space, doc)?;

  let Ok(()) = trim_text_nodes(parent_id, space, doc);
  Ok(())
}

struct TextFrame<'a, 'input: 'a> {
  children: XmlChildren<'a, 'input>,
  parent_id: NodeId,
  space: XmlSpace,
  depth: u16,
}

#[cfg_attr(
  dylint_lib = "explicit053_no_while",
  expect(
    explicit053_no_while,
    reason = "Exhausted child iterators are removed from the bounded stack of nested text elements"
  )
)]
fn parse_svg_text_element_impl<'input>(
  parent: console_core_extensible_markup::Node<'_, 'input>,
  root_id: NodeId,
  style_sheet: &console_core_cascading_style_sheets::StyleSheet,
  root_space: XmlSpace,
  doc: &mut Document<'input>,
) -> Result<(), Error> {
  let Ok(children) = parent.children();
  let mut pending = vec![TextFrame { children, parent_id: root_id, space: root_space, depth: 0 }];

  while let Some(mut frame) = pending.pop() {
    let node = match frame.children.next() {
      Some(node) => node,
      None => continue,
    };
    let TextFrame {parent_id, space, depth, ..} = frame;
    pending.push(frame);

    match ::core::primitive::bool::then_some(depth > super::parse::MAX_XML_DEPTH, ()) {
      ::core::option::Option::Some(()) => return Err(Error::NodesLimitReached),
      ::core::option::Option::None => {}
    }

    let Ok(child) = classify_text_child(node, depth);

    match child {
      TextChild::Literal(raw_text) => {
        let Ok(text) = trim_text(raw_text, space);
        let _text_id = doc.append(parent_id, NodeKind::Text(text))?;
      }
      TextChild::Reference => {
        let node_id = super::parse::parse_svg_element(
          node, parent_id, EId::Tspan, style_sheet, super::parse::IdPolicy::Preserve, doc
        )?;
        let Ok(reference_space) = get_xmlspace(doc, node_id, space);
        append_text_reference(node, node_id, reference_space, doc)?;
      }
      TextChild::Span(tag_name) => {
        let node_id = super::parse::parse_svg_element(
          node, parent_id, tag_name, style_sheet, super::parse::IdPolicy::Preserve, doc
        )?;
        let Ok(span_space) = get_xmlspace(doc, node_id, space);
        let Ok(children_2) = node.children();
        pending.push(TextFrame {
          children: children_2,
          parent_id: node_id,
          space: span_space,
          depth: depth.saturating_add(1),
        });
      }
      TextChild::Ignore => {}
    }
  }

  Ok(())
}

enum TextChild<'a> {
  Literal(&'a str),
  Reference,
  Span(EId),
  Ignore,
}

fn classify_text_child<'a>(
  node: console_core_extensible_markup::Node<'a, '_>,
  depth: u16,
) -> Result<TextChild<'a>, console_core_never::Never> {
  let Ok(is_text) = node.is_text();

  match is_text {
    console_core_extensible_markup::NodeMatch::Matches => {
      let Ok(text) = node.text();

      return Ok(match text {
        Some(text_2) => TextChild::Literal(text_2),
        None => TextChild::Ignore,
      });
    }
    console_core_extensible_markup::NodeMatch::Different => {}
  }

  let Ok(tag) = super::parse::parse_tag_name(node);

  Ok(match (tag, depth) {
    (Some(EId::Tref), _) => TextChild::Reference,
    (Some(EId::A | EId::Tspan), _) => TextChild::Span(EId::Tspan),
    (Some(EId::TextPath), 0) => TextChild::Span(EId::TextPath),
    (Some(_), _) | (None, _) => TextChild::Ignore,
  })
}

fn append_text_reference(
  node: console_core_extensible_markup::Node,
  node_id: NodeId,
  space: XmlSpace,
  doc: &mut Document,
) -> Result<(), Error> {
  let Ok(xlink) = node.attribute((XLINK_NS, "href"));
  let Ok(href) = node.attribute("href");
  let link = match xlink.or(href) {
    Some(link) => link,
    None => return Ok(()),
  };
  let Ok(document) = node.document();
  let Ok(text) = resolve_tref_text(document, link);

  match text {
    Some(text_2) => {
      let Ok(trimmed) = trim_text(&text_2, space);
      let _text_id = doc.append(node_id, NodeKind::Text(trimmed))?;
    }
    None => {}
  }

  Ok(())
}

fn resolve_tref_text(
  xml: &console_core_extensible_markup::Document,
  href: &str,
) -> core::result::Result<Option<String>, console_core_never::Never> {
  let id = match console_core_scalable_vector_graphics_values::IRI::from_str(href) {
    Ok(value) => value.0,
    Err(problem) => {
      console_core_logging::debug!("Invalid SVG text reference: {problem}");
      return Ok(None);
    }
  };

  let Ok(mut descendants) = xml.descendants();
  let node = match descendants.find(|n| {
    let Ok(attribute) = n.attribute("id");

    attribute == Some(id)
  }) {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(tag_name) = super::parse::parse_tag_name(node);

  match tag_name {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(node_descendants) = node.descendants();
  let text: String = node_descendants
    .filter(|n| {
      let Ok(is_text) = n.is_text();

      is_text == console_core_extensible_markup::NodeMatch::Matches
    })
    .filter_map(|n| {
      let Ok(n_text) = n.text();

      n_text
    })
    .collect();

  Ok(match ::core::primitive::bool::then_some(text.is_empty(), ()) {
    ::core::option::Option::Some(()) => None,
    ::core::option::Option::None => Some(text),
  })
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum XmlSpace {
  Default,
  Preserve,
}

fn get_xmlspace(
  doc: &Document,
  node_id: NodeId,
  default: XmlSpace,
) -> core::result::Result<XmlSpace, console_core_never::Never> {
  let Ok(attribute) = ({
    let Ok(explicit_value) = doc.get(node_id);
    explicit_value
  })
  .attribute(AId::Space);

  Ok(match attribute {
    Some("preserve") => XmlSpace::Preserve,
    Some(_) => XmlSpace::Default,
    ::core::option::Option::None => default,
  })
}

#[derive(Clone, Copy)]
enum TrimEdge {
  Leading,
  Trailing,
}

fn trim_leaf_edge(doc: &mut Document, id: NodeId, edge: TrimEdge) -> Result<(), Never> {
  let Ok(index) = id.get();
  let text = match doc.nodes.get_mut(match ::console_core_number_conversion::index(index) { ::core::result::Result::Ok(native_position) => native_position }).map(|node| &mut node.kind) {
    Some(NodeKind::Text(text)) => text,
    Some(NodeKind::Root | NodeKind::Element { .. }) | None => return Ok(()),
  };

  match edge {
    TrimEdge::Leading => match text.strip_prefix(' ') {
      Some(_) => {
        text.remove(0);
      }
      None => {}
    },
    TrimEdge::Trailing => match text.strip_suffix(' ') {
      Some(_) => {
        text.pop();
      }
      None => {}
    },
  }

  Ok(())
}

fn trim_text_nodes(
  text_elem_id: NodeId,
  xmlspace: XmlSpace,
  doc: &mut Document,
) -> Result<(), Never> {
  let Ok(root) = doc.get(text_elem_id);
  let mut nodes = Vec::new();
  let Ok(()) = collect_text_nodes(root, 0, &mut nodes);

  match nodes.as_slice() {
    [] => return Ok(()),
    [(only, _)] => {
      match xmlspace {
        XmlSpace::Default => {
          let Ok(()) = trim_leaf_edge(doc, *only, TrimEdge::Leading);
          let Ok(()) = trim_leaf_edge(doc, *only, TrimEdge::Trailing);
        }
        XmlSpace::Preserve => {}
      }

      return Ok(());
    }
    [_, _, ..] => {}
  }

  let mut previous_nonempty = None;
  let ::core::result::Result::Ok(last_pair) = ::console_core_number_conversion::fixed_index(nodes.len().saturating_sub(2));

  for (index, pair) in nodes.windows(2).enumerate() {
    let ((original_left, left_depth), (right, right_depth)) = match pair {
      [left, right] => (*left, *right),
      _ => continue,
    };
    let Ok(original_text) = leaf_text(doc, original_left);
    let left = match (original_text.is_empty(), previous_nonempty) {
      (true, Some(previous)) => previous,
      _ => original_left,
    };
    let Ok((left_start, left_end)) = leaf_space_edges(doc, left);
    let Ok((right_start, right_end)) = leaf_space_edges(doc, right);
    let Ok(left_space) = leaf_xmlspace(doc, left, xmlspace);
    let Ok(right_space) = leaf_xmlspace(doc, right, xmlspace);
    let Ok(()) = trim_between_leaves(doc, TextLeafPair {
      left,
      right,
      adjacency: match (left_end, right_start) {
        (_, false) => SpaceAdjacency::RightText,
        (false, true) => SpaceAdjacency::LeadingOnly,
        (true, true) => SpaceAdjacency::DoubleSpace,
      },
      left_space,
      right_space,
      ordering: left_depth.cmp(&right_depth),
    });

    let Ok(trimmed_left) = leaf_text(doc, left);

    match (index == 0 && left_start && left_space == XmlSpace::Default && !trimmed_left.is_empty(),
           index == match ::console_core_number_conversion::index(last_pair) { ::core::result::Result::Ok(native_position) => native_position } && right_end && right_space == XmlSpace::Default) {
      (true, _) => {
        let Ok(()) = trim_leaf_edge(doc, left, TrimEdge::Leading);
      }
      (false, true) => {
        let Ok(()) = trim_leaf_edge(doc, right, TrimEdge::Trailing);
      }
      (false, false) => {}
    }

    let Ok(right_text) = leaf_text(doc, right);

    match ::core::primitive::bool::then_some(index == match ::console_core_number_conversion::index(last_pair) { ::core::result::Result::Ok(native_position) => native_position } && left_end && right_text.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = trim_leaf_edge(doc, left, TrimEdge::Trailing);
      }
      ::core::option::Option::None => {}
    }

    let Ok(updated_left) = leaf_text(doc, left);

    match ::core::primitive::bool::then_some(updated_left.trim().is_empty(), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => previous_nonempty = Some(left),
    }
  }

  Ok(())
}

struct TextLeafPair {
  left: NodeId,
  right: NodeId,
  adjacency: SpaceAdjacency,
  left_space: XmlSpace,
  right_space: XmlSpace,
  ordering: core::cmp::Ordering,
}

enum SpaceAdjacency {
  RightText,
  LeadingOnly,
  DoubleSpace,
}

fn trim_between_leaves(doc: &mut Document, pair: TextLeafPair) -> Result<(), Never> {
  use core::cmp::Ordering;

  let TextLeafPair { left, right, adjacency, left_space, right_space, ordering } = pair;

  match (ordering, adjacency, left_space, right_space) {
    (Ordering::Less, SpaceAdjacency::LeadingOnly | SpaceAdjacency::DoubleSpace, _, XmlSpace::Default) => {
      trim_leaf_edge(doc, right, TrimEdge::Leading)
    }
    (Ordering::Equal | Ordering::Greater, SpaceAdjacency::DoubleSpace, XmlSpace::Default, XmlSpace::Default) => {
      trim_leaf_edge(doc, left, TrimEdge::Trailing)
    }
    (Ordering::Equal | Ordering::Greater, SpaceAdjacency::DoubleSpace, XmlSpace::Preserve, XmlSpace::Default) => {
      trim_leaf_edge(doc, right, TrimEdge::Leading)
    }
    _ => Ok(()),
  }
}

fn leaf_text<'a>(doc: &'a Document, id: NodeId) -> Result<&'a str, Never> {
  let Ok(node) = doc.get(id);

  node.text()
}

fn leaf_space_edges(doc: &Document, id: NodeId) -> Result<(bool, bool), Never> {
  let Ok(text) = leaf_text(doc, id);

  Ok((text.starts_with(' '), text.ends_with(' ')))
}

fn leaf_xmlspace(doc: &Document, id: NodeId, root_space: XmlSpace) -> Result<XmlSpace, Never> {
  let Ok(node) = doc.get(id);

  match node.parent() {
    Ok(Some(parent)) => get_xmlspace(doc, parent.id, root_space),
    Ok(None) => Ok(root_space),
  }
}

#[cfg_attr(
  dylint_lib = "explicit053_no_while",
  expect(
    explicit053_no_while,
    reason = "The stack drains at the end of each child iterator in the parsed text tree"
  )
)]
fn collect_text_nodes(
  parent: SvgNode,
  depth: u16,
  nodes: &mut Vec<(NodeId, u16)>,
) -> core::result::Result<(), console_core_never::Never> {
  let Ok(children) = parent.children();
  let mut pending = vec![(children, depth)];

  while let Some((mut children_2, current_depth)) = pending.pop() {
    let child = match children_2.next() {
      Some(child) => child,
      None => continue,
    };
    pending.push((children_2, current_depth));
    let Ok(kind) = child.is_text();
    let Ok(element) = child.is_element();

    match (kind, element) {
      (crate::NodeMatch::Matches, _) => nodes.push((child.id, current_depth)),
      (crate::NodeMatch::Different, crate::NodeMatch::Matches) => {
        let Ok(nested) = child.children();
        pending.push((nested, current_depth.saturating_add(1)));
      }
      (crate::NodeMatch::Different, crate::NodeMatch::Different) => {}
    }
  }

  Ok(())
}

fn trim_text(
  text: &str,
  space: XmlSpace,
) -> core::result::Result<String, console_core_never::Never> {
  let mut s = String::with_capacity(text.len());

  let mut prev = '0';

  for c in text.chars() {
    let c_2 = match c {
      '\r' | '\n' | '\t' => ' ',
      _ => c,
    };

    match ::core::primitive::bool::then_some(space == XmlSpace::Default && c_2 == ' ' && c_2 == prev, ()) {
      ::core::option::Option::Some(()) => {
        continue;
      }
      ::core::option::Option::None => {}
    }

    prev = c_2;

    s.push(c_2);
  }

  Ok(s)
}
