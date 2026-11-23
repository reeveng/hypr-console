use std::collections::HashMap;

use console_core_cascading_style_sheets::{
  AttributeOperator, Declaration, Matching, PseudoClass,
};
use console_core_extensible_markup::{Children as XmlChildren, Error, Node as XmlSource};
use console_core_never::Never;
use console_core_scalable_vector_graphics_values::FontShorthand;

use super::{
  AId, Attribute, DeclarationImportance, Document, EId, NodeData, NodeId, NodeKind, ShortRange,
};

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
const XML_NAMESPACE_NS: &str = "http://www.w3.org/XML/1998/namespace";

impl<'input> Document<'input> {
  pub fn parse_tree(
    xml: &console_core_extensible_markup::Document<'input>,
    injected_stylesheet: Option<&'input str>,
  ) -> Result<Document<'input>, Error> {
    parse(xml, injected_stylesheet)
  }

  pub(crate) fn append(&mut self, parent_id: NodeId, kind: NodeKind) -> Result<NodeId, Error> {
    match ::core::primitive::bool::then_some(self.nodes.len() > 1_000_000, ()) {
      ::core::option::Option::Some(()) => return Err(Error::NodesLimitReached),
      ::core::option::Option::None => {}
    }

    let Ok(count) = console_core_number_conversion::fixed_index(self.nodes.len());
    let new_child = NodeId::for_new_slot(count)?;
    let Ok(parent_index) = parent_id.get();
    let Ok(slot) = console_core_number_conversion::index(parent_index);
    let parent = self.nodes.get(slot).ok_or(Error::NoRootNode)?;
    let first_child = match parent.children {
      Some((first, previous)) => {
        let Ok(previous_index) = previous.get();
        let Ok(previous_slot) = console_core_number_conversion::index(previous_index);
        let previous_node = self.nodes.get_mut(previous_slot).ok_or(Error::NoRootNode)?;
        previous_node.next_sibling = Some(new_child);

        first
      }
      None => new_child,
    };
    let parent_2 = self.nodes.get_mut(slot).ok_or(Error::NoRootNode)?;
    parent_2.children = Some((first_child, new_child));
    self.nodes.push(NodeData {
      parent: Some(parent_id),
      next_sibling: None,
      children: None,
      kind,
    });

    Ok(new_child)
  }

  fn append_attribute(
    &mut self,
    name: AId,
    value: console_core_extensible_markup::StringStorage<'input>,
    importance: DeclarationImportance,
  ) -> core::result::Result<(), console_core_never::Never> {
    self.attrs.push(Attribute {
      name,
      value,
      importance,
    });

    Ok(())
  }
}

fn parse<'input>(
  xml: &console_core_extensible_markup::Document<'input>,
  injected_stylesheet: Option<&'input str>,
) -> Result<Document<'input>, Error> {
  let mut doc = Document {
    nodes: Vec::new(),
    attrs: Vec::new(),
    links: HashMap::new(),
  };

  let mut id_map = HashMap::new();

  let Ok(descendants) = xml.descendants();

  for node in descendants {
    let Ok(attribute) = node.attribute("id");

    match attribute {
      Some(id) => match ::core::primitive::bool::then_some(!id_map.contains_key(id), ()) {
        ::core::option::Option::Some(()) => {
          id_map.insert(id, node);
        }
        ::core::option::Option::None => {}
      },
      None => {}
    }
  }

  doc.nodes.push(NodeData {
    parent: None,
    next_sibling: None,
    children: None,
    kind: NodeKind::Root,
  });

  let Ok(style_sheet) = resolve_css(xml, injected_stylesheet);

  let Ok(root) = xml.root();
  let Ok(doc_root) = doc.root();
  let parent_id = doc_root.id;
  parse_xml_nodes(root, parent_id, &style_sheet, &mut doc, &id_map)?;

  let Ok(first_element_child) = ({
    let Ok(explicit_value) = doc.root();
    explicit_value
  })
  .first_element_child();

  match first_element_child {
    Some(child) => {
      let Ok(tag_name) = child.tag_name();

      match ::core::primitive::bool::then_some(tag_name != Some(EId::Svg), ()) {
        ::core::option::Option::Some(()) => {
          return Err(console_core_extensible_markup::Error::NoRootNode);
        }
        ::core::option::Option::None => {}
      }
    }
    None => return Err(console_core_extensible_markup::Error::NoRootNode),
  }

  let mut links = HashMap::new();

  let Ok(doc_descendants) = doc.descendants();

  for node in doc_descendants {
    let Ok(attribute) = node.attribute::<&str>(AId::Id);

    match attribute {
      Some(id) => {
        links.insert(id.to_string(), node.id);
      }
      None => {}
    }
  }

  doc.links = links;

  let Ok(()) = fix_recursive_patterns(&mut doc);
  let Ok(()) = fix_recursive_links(EId::ClipPath, AId::ClipPath, &mut doc);
  let Ok(()) = fix_recursive_links(EId::Mask, AId::Mask, &mut doc);
  let Ok(()) = fix_recursive_links(EId::Filter, AId::Filter, &mut doc);
  let Ok(()) = fix_recursive_fe_image(&mut doc);

  Ok(doc)
}

pub(crate) fn parse_tag_name(
  node: console_core_extensible_markup::Node,
) -> core::result::Result<Option<EId>, console_core_never::Never> {
  match ::core::primitive::bool::then_some(
    {
      let Ok(is_element) = node.is_element();
      !(is_element == console_core_extensible_markup::NodeMatch::Matches)
    },
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    {
      let Ok(tag_name) = node.tag_name();
      let Ok(namespace) = tag_name.namespace();
      !matches!(namespace, None | Some(SVG_NS))
    },
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let Ok(tag_name) = node.tag_name();
  let Ok(name) = tag_name.name();
  let Ok(e_id) = EId::from_str(name);

  Ok(e_id)
}

#[derive(Clone, Copy)]
pub(crate) enum IdPolicy {
  Preserve,
  Omit,
}

pub(super) const MAX_XML_DEPTH: u16 = 1024;

enum SourceNodes<'a, 'input: 'a> {
  Children(XmlChildren<'a, 'input>),
  Instance(Option<XmlSource<'a, 'input>>),
}

impl<'a, 'input: 'a> Iterator for SourceNodes<'a, 'input> {
  type Item = XmlSource<'a, 'input>;

  fn next(&mut self) -> Option<Self::Item> {
    match self {
      Self::Children(children) => children.next(),
      Self::Instance(link) => link.take(),
    }
  }
}

struct XmlFrame<'a, 'input: 'a> {
  source: SourceNodes<'a, 'input>,
  origin: XmlSource<'a, 'input>,
  parent_id: NodeId,
  ids: IdPolicy,
  depth: u16,
}

#[cfg_attr(
  dylint_lib = "explicit053_no_while",
  expect(
    explicit053_no_while,
    reason = "The heap work stack grows when an element expands and drains as its child iterator is consumed"
  )
)]
fn parse_xml_nodes<'a, 'input: 'a>(
  root: XmlSource<'a, 'input>,
  parent_id: NodeId,
  style_sheet: &console_core_cascading_style_sheets::StyleSheet,
  doc: &mut Document<'input>,
  id_map: &HashMap<&str, XmlSource<'a, 'input>>,
) -> Result<(), Error> {
  let Ok(children) = root.children();
  let mut pending = vec![XmlFrame {
    source: SourceNodes::Children(children),
    origin: root,
    parent_id,
    ids: IdPolicy::Preserve,
    depth: 0,
  }];

  while let Some(mut frame) = pending.pop() {
    let node = match frame.source.next() {
      Some(node) => node,
      None => continue,
    };
    let Ok(expansion) = parse_tag_name(node);

    match (frame.depth > MAX_XML_DEPTH).then_some(()) {
      Some(()) => return Err(Error::NodesLimitReached),
      None => {}
    }

    let tag_name = match expansion {
      Some(EId::Style) | None => {
        pending.push(frame);
        continue;
      }
      Some(EId::A) => EId::G,
      Some(tag_name) => tag_name,
    };
    let node_id =
      parse_svg_element(node, frame.parent_id, tag_name, style_sheet, frame.ids, doc)?;
    let child_frame = xml_child_frame(node, node_id, tag_name, &frame, id_map, style_sheet, doc)?;
    pending.push(frame);

    match child_frame {
      Some(children_2) => pending.push(children_2),
      None => {}
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit016_no_wildcard_arm",
  expect(
    explicit016_no_wildcard_arm,
    reason = "Text and use need special child handling, and all other SVG elements traverse their XML children"
  )
)]
fn xml_child_frame<'a, 'input: 'a>(
  node: XmlSource<'a, 'input>,
  parent_id: NodeId,
  tag: EId,
  parent: &XmlFrame<'a, 'input>,
  id_map: &HashMap<&str, XmlSource<'a, 'input>>,
  style_sheet: &console_core_cascading_style_sheets::StyleSheet,
  doc: &mut Document<'input>,
) -> Result<Option<XmlFrame<'a, 'input>>, Error> {
  let depth = parent.depth.saturating_add(1);

  match tag {
    EId::Text => {
      super::text::parse_svg_text_element(node, parent_id, style_sheet, doc)?;
      Ok(None)
    }
    EId::Use => {
      let Ok(target) = resolve_svg_use_target(node, parent.origin, id_map);

      Ok(target.map(|link| XmlFrame {
        source: SourceNodes::Instance(Some(link)),
        origin: node,
        parent_id,
        ids: IdPolicy::Omit,
        depth: depth.saturating_add(1),
      }))
    }
    _ => {
      let Ok(children) = node.children();

      Ok(Some(XmlFrame {
        source: SourceNodes::Children(children),
        origin: parent.origin,
        parent_id,
        ids: parent.ids,
        depth,
      }))
    }
  }
}

pub(crate) fn parse_svg_element<'input>(
  xml_node: XmlSource<'_, 'input>,
  parent_id: NodeId,
  tag_name: EId,
  style_sheet: &console_core_cascading_style_sheets::StyleSheet,
  ids: IdPolicy,
  doc: &mut Document<'input>,
) -> Result<NodeId, Error> {
  let range_start =
    u32::try_from(doc.attrs.len()).map_err(|_oversized| Error::NodesLimitReached)?;
  let mut attributes = ElementAttributes {
    doc,
    start: range_start,
    parent: parent_id,
    tag: tag_name,
  };
  let Ok(()) = attributes.read_literal(xml_node, ids);
  let Ok(()) = attributes.apply_style(xml_node, style_sheet);
  let range_end =
    u32::try_from(attributes.doc.attrs.len()).map_err(|_oversized| Error::NodesLimitReached)?;
  let range = ShortRange {
    start: range_start,
    end: range_end,
  };

  attributes.doc.append(
    parent_id,
    NodeKind::Element {
      tag_name,
      attributes: range,
    },
  )
}

struct ElementAttributes<'a, 'input> {
  doc: &'a mut Document<'input>,
  start: u32,
  parent: NodeId,
  tag: EId,
}

impl<'input> ElementAttributes<'_, 'input> {
  fn read_literal(
    &mut self,
    xml_node: XmlSource<'_, 'input>,
    ids: IdPolicy,
  ) -> Result<(), Never> {
    let mut href = None;
    let Ok(attributes) = xml_node.attributes();

    for attribute in attributes {
      let Ok(namespace) = attribute.namespace();

      match namespace {
        None | Some(SVG_NS | XLINK_NS | XML_NAMESPACE_NS) => {}
        Some(_) => continue,
      }

      let Ok(name) = attribute.name();
      let aid = match AId::from_str(name) {
        Ok(Some(aid)) => aid,
        Ok(None) => continue,
      };
      let Ok(value) = attribute.value();

      match (ids, aid, value) {
        (IdPolicy::Omit, AId::Id, _)
        | (_, AId::MixBlendMode | AId::Isolation | AId::FontKerning, _)
        | (_, AId::ImageRendering, "smooth" | "high-quality" | "crisp-edges" | "pixelated") => {
          continue;
        }
        (_, _, _) => {}
      }

      let Ok(storage) = attribute.value_storage();

      match (aid, namespace, href) {
        (AId::Href, None, Some(index)) => {
          let Ok(href_index) = console_core_number_conversion::index(index);

          match self.doc.attrs.get_mut(href_index) {
            Some(pending) => pending.value = storage.clone(),
            None => {}
          }

          continue;
        }
        (AId::Href, Some(XLINK_NS), Some(_))
        | (AId::Href, Some(SVG_NS | XML_NAMESPACE_NS), _) => continue,
        (_, _, _) => {}
      }

      let Ok(added) = append_attribute(
        self.parent,
        self.tag,
        aid,
        storage.clone(),
        DeclarationImportance::Normal,
        self.doc,
      );

      match (added, aid) {
        (crate::AppendAttributeAnswer::Yes, AId::Href) => {
          let Ok(count) = console_core_number_conversion::fixed_index(self.doc.attrs.len());
          href = count.checked_sub(1);
        }
        (crate::AppendAttributeAnswer::Yes | crate::AppendAttributeAnswer::No, _) => {}
      }
    }

    Ok(())
  }

  fn apply_style(
    &mut self,
    xml_node: XmlSource<'_, 'input>,
    style_sheet: &console_core_cascading_style_sheets::StyleSheet,
  ) -> Result<(), Never> {
    use console_core_cascading_style_sheets::DeclarationTokenizer;

    for rule in &style_sheet.rules {
      match rule.selector.matches(&XmlNode(xml_node)) {
        Ok(Matching::Yes) => {
          for declaration in &rule.declarations {
            let Ok(()) = self.write_declaration(declaration);
          }
        }
        Ok(Matching::No) => {}
      }
    }

    let Ok(inline) = xml_node.attribute("style");

    match inline {
      Some(style) => {
        for declaration in DeclarationTokenizer::from(style) {
          let Ok(()) = self.write_declaration(&declaration);
        }
      }
      None => {}
    }

    Ok(())
  }

  fn write_declaration(&mut self, declaration: &Declaration) -> Result<(), Never> {
    let importance = DeclarationImportance::from(declaration.important);
    let value = declaration.value;

    match declaration.name {
      "marker" => {
        for attribute in [AId::MarkerStart, AId::MarkerMid, AId::MarkerEnd] {
          let Ok(()) = self.insert_style(attribute, value, importance);
        }
      }
      "font" => {
        let Ok(()) = self.write_font_shorthand(value, importance);
      }
      name => {
        let Ok(attribute) = AId::from_str(name);

        match attribute {
          Some(attribute_2) => match attribute_2.is_presentation() {
            Ok(crate::IsPresentationAnswer::Yes) => {
              let Ok(()) = self.insert_style(attribute_2, value, importance);
            }
            Ok(crate::IsPresentationAnswer::No) => {}
          },
          None => {}
        }
      }
    }

    Ok(())
  }

  fn write_font_shorthand(
    &mut self,
    value: &str,
    importance: DeclarationImportance,
  ) -> Result<(), Never> {
    let shorthand = match FontShorthand::from_str(value) {
      Ok(shorthand) => shorthand,
      Err(problem) => {
        console_core_logging::warn!("Invalid SVG font shorthand {value:?}: {problem}");
        return Ok(());
      }
    };

    for (attribute, fallback) in [
      (AId::FontStyle, "normal"),
      (AId::FontVariant, "normal"),
      (AId::FontWeight, "normal"),
      (AId::FontStretch, "normal"),
      (AId::LineHeight, "normal"),
      (AId::FontSizeAdjust, "none"),
      (AId::FontKerning, "auto"),
      (AId::FontVariantCaps, "normal"),
      (AId::FontVariantLigatures, "normal"),
      (AId::FontVariantNumeric, "normal"),
      (AId::FontVariantEastAsian, "normal"),
      (AId::FontVariantPosition, "normal"),
    ] {
      let Ok(()) = self.insert_style(attribute, fallback, importance);
    }

    for (attribute, authored) in [
      (AId::FontStretch, shorthand.font_stretch),
      (AId::FontWeight, shorthand.font_weight),
      (AId::FontVariant, shorthand.font_variant),
      (AId::FontStyle, shorthand.font_style),
      (AId::FontSize, Some(shorthand.font_size)),
      (AId::FontFamily, Some(shorthand.font_family)),
    ] {
      match authored {
        Some(authored_2) => {
          let Ok(()) = self.insert_style(attribute, authored_2, importance);
        }
        None => {}
      }
    }

    Ok(())
  }

  fn insert_style(
    &mut self,
    id: AId,
    value: &str,
    importance: DeclarationImportance,
  ) -> Result<(), Never> {
    let Ok(start) = console_core_number_conversion::index(self.start);
    let existing = self.doc.attrs.get(start..).and_then(|attributes| {
      let (_, position) = attributes
        .iter()
        .zip(u64::from(self.start)..)
        .find(|(attribute, _)| attribute.name == id)?;

      Some(position)
    });
    let Ok(storage) = console_core_extensible_markup::StringStorage::new_owned(value);
    let Ok(appended) = append_attribute(self.parent, self.tag, id, storage, importance, self.doc);

    match (appended, existing) {
      (crate::AppendAttributeAnswer::Yes, Some(position)) => {
        let Ok(()) = self.replace_previous_style(position);
      }
      (crate::AppendAttributeAnswer::Yes, None) | (crate::AppendAttributeAnswer::No, _) => {}
    }

    Ok(())
  }

  fn replace_previous_style(&mut self, position: u64) -> Result<(), Never> {
    let replacement = match self.doc.attrs.pop() {
      Some(attribute) => attribute,
      None => return Ok(()),
    };
    let Ok(index) = console_core_number_conversion::index(position);

    match self.doc.attrs.get_mut(index) {
      Some(previous) => match previous.importance {
        DeclarationImportance::Normal => *previous = replacement,
        DeclarationImportance::Important => {}
      },
      None => self.doc.attrs.push(replacement),
    }

    Ok(())
  }
}

fn append_attribute<'input>(
  parent_id: NodeId,
  tag_name: EId,
  aid: AId,
  value: console_core_extensible_markup::StringStorage<'input>,
  importance: DeclarationImportance,
  doc: &mut Document<'input>,
) -> core::result::Result<crate::AppendAttributeAnswer, console_core_never::Never> {
  match (tag_name, aid) {
    (_, AId::Style | AId::Class) | (EId::Tspan, AId::Href) => {
      return Ok(crate::AppendAttributeAnswer::No);
    }
    (_, _) => {}
  }

  let Ok(allows_inherit_value) = aid.allows_inherit_value();

  match ::core::primitive::bool::then_some(
    matches!(allows_inherit_value, crate::AllowsInheritValueAnswer::Yes) && &*value == "inherit",
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(
        match {
          let Ok(explicit_value) = resolve_inherit(parent_id, aid, doc);
          explicit_value
        } {
          crate::ResolveInheritAnswer::Yes => crate::AppendAttributeAnswer::Yes,
          crate::ResolveInheritAnswer::No => crate::AppendAttributeAnswer::No,
        },
      );
    }
    ::core::option::Option::None => {}
  }

  let Ok(()) = doc.append_attribute(aid, value, importance);

  Ok(match ::core::primitive::bool::then_some(true, ()) {
    ::core::option::Option::Some(()) => crate::AppendAttributeAnswer::Yes,
    ::core::option::Option::None => crate::AppendAttributeAnswer::No,
  })
}

#[cfg_attr(
  dylint_lib = "explicit016_no_wildcard_arm",
  expect(
    explicit016_no_wildcard_arm,
    reason = "Only the presentation attributes with declared initial values have a fallback when no ancestor supplies a value"
  )
)]
fn resolve_inherit(
  parent_id: NodeId,
  aid: AId,
  doc: &mut Document,
) -> core::result::Result<crate::ResolveInheritAnswer, console_core_never::Never> {
  let Ok(parent) = doc.get(parent_id);
  let Ok(inheritable) = aid.is_inheritable();
  let source = match inheritable {
    crate::IsInheritableAnswer::Yes => {
      let Ok(mut ancestors) = parent.ancestors();
      ancestors.find(|node| matches!(node.has_attribute(aid), Ok(crate::Presence::Present)))
    }
    crate::IsInheritableAnswer::No => Some(parent),
  };
  let inherited = match source {
    Some(node) => {
      let Ok(attributes) = node.attributes();
      attributes
        .iter()
        .find(|attribute| attribute.name == aid)
        .cloned()
    }
    None => None,
  };

  match inherited {
    Some(attribute) => {
      doc.attrs.push(attribute);

      return Ok(crate::ResolveInheritAnswer::Yes);
    }
    None => {}
  }

  let value = match aid {
    AId::ImageRendering | AId::ShapeRendering | AId::TextRendering => "auto",

    AId::ClipPath
    | AId::Filter
    | AId::MarkerEnd
    | AId::MarkerMid
    | AId::MarkerStart
    | AId::Mask
    | AId::Stroke
    | AId::StrokeDasharray
    | AId::TextDecoration => "none",

    AId::FontStretch
    | AId::FontStyle
    | AId::FontVariant
    | AId::FontWeight
    | AId::LetterSpacing
    | AId::WordSpacing => "normal",

    AId::Fill | AId::FloodColor | AId::StopColor => "black",

    AId::FillOpacity
    | AId::FloodOpacity
    | AId::Opacity
    | AId::StopOpacity
    | AId::StrokeOpacity => "1",

    AId::ClipRule | AId::FillRule => "nonzero",

    AId::BaselineShift => "baseline",
    AId::ColorInterpolationFilters => "linearRGB",
    AId::Direction => "ltr",
    AId::Display => "inline",
    AId::FontSize => "medium",
    AId::Overflow => "visible",
    AId::StrokeDashoffset => "0",
    AId::StrokeLinecap => "butt",
    AId::StrokeLinejoin => "miter",
    AId::StrokeMiterlimit => "4",
    AId::StrokeWidth => "1",
    AId::TextAnchor => "start",
    AId::Visibility => "visible",
    AId::WritingMode => "lr-tb",
    _ => return Ok(crate::ResolveInheritAnswer::No),
  };

  let Ok(()) = doc.append_attribute(
    aid,
    console_core_extensible_markup::StringStorage::Borrowed(value),
    DeclarationImportance::Normal,
  );

  Ok(crate::ResolveInheritAnswer::Yes)
}

fn resolve_href<'a, 'input: 'a>(
  node: console_core_extensible_markup::Node<'a, 'input>,
  id_map: &HashMap<&str, console_core_extensible_markup::Node<'a, 'input>>,
) -> core::result::Result<
  Option<console_core_extensible_markup::Node<'a, 'input>>,
  console_core_never::Never,
> {
  let Ok(mut attributes) = node.attributes();
  let link_value = match attributes
    .find(|a| {
      let Ok(name) = a.name();

      name == "href"
        && ({
          let Ok(explicit_value) = a.namespace();
          explicit_value
        })
        .is_none()
    })
    .or_else(|| {
      let Ok(mut node_attributes) = node.attributes();

      node_attributes.find(|a| {
        let Ok(name) = a.name();

        name == "href"
          && ({
            let Ok(explicit_value) = a.namespace();
            explicit_value
          }) == Some(XLINK_NS)
      })
    })
    .map(|a| {
      let Ok(explicit_value) = a.value();
      explicit_value
    }) {
    Some(value) => value,
    None => return Ok(None),
  };

  let link_id = match console_core_scalable_vector_graphics_values::IRI::from_str(link_value) {
    Ok(value) => value.0,
    Err(problem) => {
      console_core_logging::debug!("Invalid SVG use reference: {problem}");

      return Ok(None);
    }
  };

  Ok(id_map.get(link_id).copied())
}

fn resolve_svg_use_target<'a, 'input: 'a>(
  node: XmlSource<'a, 'input>,
  origin: XmlSource<'a, 'input>,
  id_map: &HashMap<&str, XmlSource<'a, 'input>>,
) -> Result<Option<XmlSource<'a, 'input>>, console_core_never::Never> {
  let Ok(reference) = resolve_href(node, id_map);
  let link = match reference {
    Some(link) => link,
    None => return Ok(None),
  };
  let Ok(tag_name) = parse_tag_name(link);
  let Ok(descendants) = link.descendants();

  match ::core::primitive::bool::then_some(link == node || link == origin, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(()) = log_recursive_use(node);
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  match tag_name {
    Some(_) => {}
    None => return Ok(None),
  }

  let points_back = descendants.skip(1).any(|child| {
    let Ok(has_tag_name) = child.has_tag_name((SVG_NS, "use"));

    match has_tag_name {
      console_core_extensible_markup::NodeMatch::Different => false,
      console_core_extensible_markup::NodeMatch::Matches => {
        let Ok(target) = resolve_href(child, id_map);

        matches!(target, Some(target_2) if target_2 == node || target_2 == link)
      }
    }
  });

  match ::core::primitive::bool::then_some(points_back, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(()) = log_recursive_use(node);
      Ok(None)
    }
    ::core::option::Option::None => Ok(Some(link)),
  }
}

fn log_recursive_use(node: XmlSource<'_, '_>) -> Result<(), console_core_never::Never> {
  let Ok(attribute) = node.attribute((SVG_NS, "id"));

  match attribute {
    Some(id) => console_core_logging::warn!("Skipping recursive SVG use {id:?}"),
    None => console_core_logging::warn!("Skipping recursive SVG use"),
  }

  Ok(())
}

fn resolve_css<'a>(
  xml: &'a console_core_extensible_markup::Document<'a>,
  style_sheet: Option<&'a str>,
) -> core::result::Result<
  console_core_cascading_style_sheets::StyleSheet<'a>,
  console_core_never::Never,
> {
  let mut sheet = console_core_cascading_style_sheets::StyleSheet::default();

  match style_sheet {
    Some(style_sheet_2) => {
      let Ok(()) = sheet.parse_more(style_sheet_2);
    }
    None => {}
  }

  let Ok(descendants) = xml.descendants();

  for node in descendants.filter(|n| {
    let Ok(has_tag_name) = n.has_tag_name("style");

    has_tag_name == console_core_extensible_markup::NodeMatch::Matches
  }) {
    let Ok(attribute) = node.attribute("type");

    match attribute {
      Some("text/css") => {}
      Some(_) => continue,
      None => {}
    }

    let Ok(node_text) = node.text();
    let text = match node_text {
      Some(v) => v,
      None => continue,
    };

    let Ok(()) = sheet.parse_more(text);
  }

  Ok(sheet)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct XmlNode<'a, 'input: 'a>(console_core_extensible_markup::Node<'a, 'input>);

impl console_core_cascading_style_sheets::Element for XmlNode<'_, '_> {
  fn ancestor_elements(&self) -> impl Iterator<Item = Self> {
    let Ok(ancestors) = self.0.ancestors();

    ancestors
      .skip(1)
      .filter(|node| {
        let Ok(element) = node.is_element();

        element == console_core_extensible_markup::NodeMatch::Matches
      })
      .map(XmlNode)
  }

  fn prev_sibling_element(&self) -> Option<Self> {
    let Ok(previous) = self.0.prev_sibling_element();

    previous.map(XmlNode)
  }

  fn has_local_name(&self, local_name: &str) -> Matching {
    let Ok(tag) = self.0.tag_name();
    let Ok(name) = tag.name();

    Matching::from(name == local_name)
  }

  fn attribute_matches(&self, local_name: &str, operator: AttributeOperator<'_>) -> Matching {
    let Ok(attribute) = self.0.attribute(local_name);

    match attribute {
      Some(value) => {
        let Ok(matched) = operator.matches(value);

        matched
      }
      None => Matching::No,
    }
  }

  fn pseudo_class_matches(&self, class: PseudoClass<'_>) -> Matching {
    match class {
      PseudoClass::FirstChild => Matching::from(self.prev_sibling_element().is_none()),
      PseudoClass::Link
      | PseudoClass::Visited
      | PseudoClass::Hover
      | PseudoClass::Active
      | PseudoClass::Focus
      | PseudoClass::Lang(_) => Matching::No,
    }
  }
}

fn fix_recursive_patterns(doc: &mut Document) -> Result<(), console_core_never::Never> {
  let Ok(reference_budget) = console_core_number_conversion::fixed_index(doc.attrs.len());

  for attribute in [AId::Fill, AId::Stroke] {
    'inner_for_2: for _ in 0..reference_budget {
      let Ok(recursive) = find_recursive_pattern(attribute, doc);
      let node = match recursive {
        Some(node) => node,
        None => break 'inner_for_2,
      };

      match doc.clear_reference(node, attribute) {
        Ok(ReferenceChange::Removed) => {}
        Ok(ReferenceChange::Absent) => break 'inner_for_2,
      }
    }
  }

  Ok(())
}

enum ReferenceChange {
  Removed,
  Absent,
}

impl Document<'_> {
  fn clear_reference(
    &mut self,
    id: NodeId,
    attribute: AId,
  ) -> Result<ReferenceChange, console_core_never::Never> {
    let Ok(node) = self.get(id);
    let Ok(position) = node.attribute_id(attribute);
    let index = match position {
      Some(index) => index,
      None => return Ok(ReferenceChange::Absent),
    };
    let Ok(slot) = console_core_number_conversion::index(index);
    let entry = match self.attrs.get_mut(slot) {
      Some(entry) => entry,
      None => return Ok(ReferenceChange::Absent),
    };

    entry.value = console_core_extensible_markup::StringStorage::Borrowed("none");

    Ok(ReferenceChange::Removed)
  }
}

fn paint_reference<'a>(
  node: super::SvgNode<'a, '_>,
  attribute: AId,
) -> Result<Option<&'a str>, console_core_never::Never> {
  use console_core_scalable_vector_graphics_values::Paint;

  let Ok(authored) = node.attribute(attribute);
  let value = match authored {
    Some(value) => value,
    None => return Ok(None),
  };

  match Paint::from_str(value) {
    Ok(Paint::FuncIRI(link, _)) => Ok(Some(link)),
    Ok(
      Paint::None
      | Paint::Inherit
      | Paint::CurrentColor
      | Paint::Color(_)
      | Paint::ContextFill
      | Paint::ContextStroke,
    ) => Ok(None),
    Err(problem) => {
      console_core_logging::debug!("Invalid SVG paint reference: {problem}");

      Ok(None)
    }
  }
}

fn pattern_reference(
  pattern: super::SvgNode,
  node: super::SvgNode,
  attribute: AId,
  doc: &Document,
) -> Result<Option<NodeId>, console_core_never::Never> {
  let Ok(reference) = paint_reference(node, attribute);
  let link = match reference {
    Some(link) => link,
    None => return Ok(None),
  };
  let Ok(pattern_id) = pattern.element_id();

  match link.cmp(pattern_id) {
    core::cmp::Ordering::Equal => return Ok(Some(node.id)),
    core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {}
  }

  let Ok(target) = doc.element_by_id(link);
  let linked = match target {
    Some(target_node) => target_node,
    None => return Ok(None),
  };
  let Ok(descendants) = linked.descendants();

  for descendant in descendants {
    let Ok(back_reference) = paint_reference(descendant, attribute);

    match back_reference.map(|target_id| target_id.cmp(pattern_id)) {
      Some(core::cmp::Ordering::Equal) => return Ok(Some(descendant.id)),
      Some(core::cmp::Ordering::Less | core::cmp::Ordering::Greater) | None => {}
    }
  }

  Ok(None)
}

fn find_recursive_pattern(
  attribute: AId,
  doc: &Document,
) -> Result<Option<NodeId>, console_core_never::Never> {
  let Ok(root) = doc.root();
  let Ok(descendants) = root.descendants();

  for pattern in descendants.filter(|node| matches!(node.tag_name(), Ok(Some(EId::Pattern)))) {
    let Ok(children) = pattern.descendants();

    for child in children {
      let Ok(recursive) = pattern_reference(pattern, child, attribute, doc);

      match recursive {
        Some(node) => return Ok(Some(node)),
        None => {}
      }
    }
  }

  Ok(None)
}

fn fix_recursive_links(
  element: EId,
  attribute: AId,
  doc: &mut Document,
) -> Result<(), console_core_never::Never> {
  let Ok(reference_budget) = console_core_number_conversion::fixed_index(doc.attrs.len());

  for _ in 0..reference_budget {
    let Ok(recursive) = find_recursive_link(element, attribute, doc);
    let node = match recursive {
      Some(node) => node,
      None => break,
    };

    match doc.clear_reference(node, attribute) {
      Ok(ReferenceChange::Removed) => {}
      Ok(ReferenceChange::Absent) => break,
    }
  }

  Ok(())
}

fn find_recursive_link(
  element: EId,
  attribute: AId,
  doc: &Document,
) -> Result<Option<NodeId>, console_core_never::Never> {
  let Ok(nodes) = doc.descendants();

  for node in nodes.filter(|candidate| candidate.tag_name() == Ok(Some(element))) {
    let Ok(children) = node.descendants();

    'over_child_2: for child in children {
      let linked = match child.node_attribute(attribute) {
        Ok(Some(linked)) => linked,
        Ok(None) => continue 'over_child_2,
      };

      match ::core::primitive::bool::then_some(linked == node, ()) {
        ::core::option::Option::Some(()) => return Ok(Some(child.id)),
        ::core::option::Option::None => {}
      }

      let Ok(mut references) = linked.descendants();
      let back_link =
        references.find(|reference| reference.node_attribute(attribute) == Ok(Some(node)));

      match back_link {
        Some(reference) => return Ok(Some(reference.id)),
        None => {}
      }
    }
  }

  Ok(None)
}

fn fix_recursive_fe_image(doc: &mut Document) -> Result<(), console_core_never::Never> {
  use console_core_scalable_vector_graphics_values::{FilterValue, FilterValueListParser};

  let mut recursive_images = Vec::new();
  let Ok(nodes) = doc.descendants();

  for image in nodes.filter(|node| node.tag_name() == Ok(Some(EId::FeImage))) {
    let source = match image.node_attribute(AId::Href) {
      Ok(Some(source)) => source,
      Ok(None) => continue,
    };
    let uri = match source.attribute::<&str>(AId::Filter) {
      Ok(Some(uri)) => uri,
      Ok(None) => continue,
    };
    let parent = match image.parent() {
      Ok(Some(parent)) => parent,
      Ok(None) => continue,
    };
    let Ok(filter_id) = parent.element_id();

    for parsed in FilterValueListParser::from(uri) {
      match parsed {
        Ok(FilterValue::Url(url)) => match url.cmp(filter_id) {
          core::cmp::Ordering::Equal => recursive_images.push(source.id),
          core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {}
        },
        Ok(_) => {}
        Err(problem) => {
          console_core_logging::debug!("Ignoring malformed SVG filter link {uri:?}: {problem}");
        }
      }
    }
  }

  for id in recursive_images {
    let Ok(_change) = doc.clear_reference(id, AId::Filter);
  }

  Ok(())
}
