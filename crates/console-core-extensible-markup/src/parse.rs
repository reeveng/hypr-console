use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::{vec, vec::Vec};
use core::cmp::Ordering;
use core::fmt;
use core::mem::take;
use core::ops::Range;
use memchr::{memchr_iter, memchr2};

use crate::{
  AttributeData, Document, ExpandedNameIndexed, NS_XML_PREFIX, NS_XML_URI, NS_XMLNS_URI,
  NamespaceIdx, Namespaces, NodeData, NodeId, NodeKind, PI, ShortRange, StringStorage, TextPos,
  XMLNS,
};

use crate::tokenizer::{self, Reference, StrSpan, Stream};

type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
  InvalidXmlPrefixUri(TextPos),

  UnexpectedXmlUri(TextPos),

  UnexpectedXmlnsUri(TextPos),

  InvalidElementNamePrefix(TextPos),

  DuplicatedNamespace(String, TextPos),

  UnknownNamespace(String, TextPos),

  UnexpectedCloseTag(String, String, TextPos),

  UnexpectedEntityCloseTag(TextPos),

  UnknownEntityReference(String, TextPos),

  MalformedEntityReference(TextPos),

  EntityReferenceLoop(TextPos),

  InvalidAttributeValue(TextPos),

  DuplicatedAttribute(String, TextPos),

  NoRootNode,

  InvalidNormalizedText,

  InvalidNamespaceIndex,

  MissingTextNode,

  MissingParentPrefix,

  InvalidAttributeIndex,

  RangeTooLarge,

  UnclosedRootNode,

  UnexpectedDeclaration(TextPos),

  DtdDetected,

  NodesLimitReached,

  AttributesLimitReached,

  NamespacesLimitReached,

  InvalidName(TextPos),

  NonXmlChar(char, TextPos),

  InvalidChar(u8, u8, TextPos),

  InvalidChar2(&'static str, u8, TextPos),

  InvalidString(&'static str, TextPos),

  InvalidExternalID(TextPos),

  EntityResolver(TextPos, ResolutionError),

  InvalidComment(TextPos),

  InvalidCharacterData(TextPos),

  UnknownToken(TextPos),

  UnexpectedEndOfStream,
}

impl Error {
  pub fn pos(&self) -> core::result::Result<TextPos, console_core_never::Never> {
    Ok(match *self {
      Error::InvalidXmlPrefixUri(pos) => pos,
      Error::UnexpectedXmlUri(pos) => pos,
      Error::UnexpectedXmlnsUri(pos) => pos,
      Error::InvalidElementNamePrefix(pos) => pos,
      Error::DuplicatedNamespace(_, pos) => pos,
      Error::UnknownNamespace(_, pos) => pos,
      Error::UnexpectedCloseTag(_, _, pos) => pos,
      Error::UnexpectedEntityCloseTag(pos) => pos,
      Error::UnknownEntityReference(_, pos) => pos,
      Error::MalformedEntityReference(pos) => pos,
      Error::EntityReferenceLoop(pos) => pos,
      Error::InvalidAttributeValue(pos) => pos,
      Error::DuplicatedAttribute(_, pos) => pos,
      Error::NoRootNode
      | Error::InvalidNormalizedText
      | Error::InvalidNamespaceIndex
      | Error::MissingTextNode
      | Error::MissingParentPrefix
      | Error::InvalidAttributeIndex
      | Error::RangeTooLarge => {
        let Ok(text_pos) = TextPos::new(crate::Row(1), crate::Column(1));

        text_pos
      }
      Error::UnclosedRootNode => {
        let Ok(text_pos) = TextPos::new(crate::Row(1), crate::Column(1));

        text_pos
      }
      Error::UnexpectedDeclaration(pos) => pos,
      Error::DtdDetected => {
        let Ok(text_pos) = TextPos::new(crate::Row(1), crate::Column(1));

        text_pos
      }
      Error::NodesLimitReached => {
        let Ok(text_pos) = TextPos::new(crate::Row(1), crate::Column(1));

        text_pos
      }
      Error::AttributesLimitReached => {
        let Ok(text_pos) = TextPos::new(crate::Row(1), crate::Column(1));

        text_pos
      }
      Error::NamespacesLimitReached => {
        let Ok(text_pos) = TextPos::new(crate::Row(1), crate::Column(1));

        text_pos
      }
      Error::InvalidName(pos) => pos,
      Error::NonXmlChar(_, pos) => pos,
      Error::InvalidChar(_, _, pos) => pos,
      Error::InvalidChar2(_, _, pos) => pos,
      Error::InvalidString(_, pos) => pos,
      Error::InvalidExternalID(pos) => pos,
      Error::EntityResolver(pos, _) => pos,
      Error::InvalidComment(pos) => pos,
      Error::InvalidCharacterData(pos) => pos,
      Error::UnknownToken(pos) => pos,
      Error::UnexpectedEndOfStream => {
        let Ok(text_pos) = TextPos::new(crate::Row(1), crate::Column(1));

        text_pos
      }
    })
  }
}

impl core::fmt::Display for Error {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    match self {
      Error::InvalidXmlPrefixUri(pos) => {
        write!(f, "'xml' namespace prefix mapped to wrong URI at {}", pos)
      }
      Error::UnexpectedXmlUri(pos) => {
        write!(
          f,
          "the 'xml' namespace URI is used for not 'xml' prefix at {}",
          pos
        )
      }
      Error::UnexpectedXmlnsUri(pos) => {
        write!(
          f,
          "the 'xmlns' URI is used at {}, but it must not be declared",
          pos
        )
      }
      Error::InvalidElementNamePrefix(pos) => {
        write!(
          f,
          "the 'xmlns' prefix is used at {}, but it must not be",
          pos
        )
      }
      Error::DuplicatedNamespace(name, pos) => {
        write!(f, "namespace '{}' at {} is already defined", name, pos)
      }
      Error::UnknownNamespace(name, pos) => {
        write!(f, "an unknown namespace prefix '{}' at {}", name, pos)
      }
      Error::UnexpectedCloseTag(expected, actual, pos) => {
        write!(
          f,
          "expected '{}' tag, not '{}' at {}",
          expected, actual, pos
        )
      }
      Error::UnexpectedEntityCloseTag(pos) => {
        write!(f, "unexpected close tag at {}", pos)
      }
      Error::MalformedEntityReference(pos) => {
        write!(f, "malformed entity reference at {}", pos)
      }
      Error::UnknownEntityReference(name, pos) => {
        write!(f, "unknown entity reference '{}' at {}", name, pos)
      }
      Error::EntityReferenceLoop(pos) => {
        write!(f, "a possible entity reference loop is detected at {}", pos)
      }
      Error::InvalidAttributeValue(pos) => {
        write!(f, "unescaped '<' found at {}", pos)
      }
      Error::DuplicatedAttribute(name, pos) => {
        write!(f, "attribute '{}' at {} is already defined", name, pos)
      }
      Error::InvalidNormalizedText => write!(f, "normalized XML text is not UTF-8"),
      Error::InvalidNamespaceIndex => write!(f, "XML namespace index is invalid"),
      Error::MissingTextNode => write!(f, "XML text continuation has no text node"),
      Error::MissingParentPrefix => write!(f, "XML closing tag has no parent prefix"),
      Error::InvalidAttributeIndex => write!(f, "XML attribute index is invalid"),
      Error::RangeTooLarge => write!(f, "XML source range exceeds the stored range width"),
      Error::NoRootNode => {
        write!(f, "the document does not have a root node")
      }
      Error::UnclosedRootNode => {
        write!(f, "the root node was opened but never closed")
      }
      Error::UnexpectedDeclaration(pos) => {
        write!(f, "unexpected XML declaration at {}", pos)
      }
      Error::DtdDetected => {
        write!(f, "XML with DTD detected")
      }
      Error::NodesLimitReached => {
        write!(f, "nodes limit reached")
      }
      Error::AttributesLimitReached => {
        write!(f, "more than 2^32 attributes were parsed")
      }
      Error::NamespacesLimitReached => {
        write!(f, "more than 2^16 unique namespaces were parsed")
      }
      Error::InvalidName(pos) => {
        write!(f, "invalid name token at {}", pos)
      }
      Error::NonXmlChar(c, pos) => {
        write!(f, "a non-XML character {:?} found at {}", c, pos)
      }
      Error::InvalidChar(expected, actual, pos) => {
        write!(
          f,
          "expected '{}' not '{}' at {}",
          char::from(*expected),
          char::from(*actual),
          pos
        )
      }
      Error::InvalidChar2(expected, actual, pos) => {
        write!(
          f,
          "expected {} not '{}' at {}",
          expected,
          char::from(*actual),
          pos
        )
      }
      Error::InvalidString(expected, pos) => {
        write!(f, "expected '{}' at {}", expected, pos)
      }
      Error::InvalidExternalID(pos) => {
        write!(f, "invalid ExternalID at {}", pos)
      }
      Error::EntityResolver(pos, msg) => {
        write!(f, "entity resolver failed at {}: {}", pos, msg)
      }
      Error::InvalidComment(pos) => {
        write!(f, "comment at {} contains '--'", pos)
      }
      Error::InvalidCharacterData(pos) => {
        write!(f, "']]>' at {} is not allowed inside a character data", pos)
      }
      Error::UnknownToken(pos) => {
        write!(f, "unknown token at {}", pos)
      }
      Error::UnexpectedEndOfStream => {
        write!(f, "unexpected end of stream")
      }
    }
  }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {
  fn description(&self) -> &str {
    "an XML parsing error"
  }
}

pub struct ParsingOptions<'input> {
  pub dtd: crate::Dtd,

  pub nodes_limit: u32,

  pub entity_resolver: Option<&'input EntityResolver<'input>>,
}

pub type ResolutionError = alloc::boxed::Box<dyn core::error::Error + Send + Sync>;

pub type EntityResolver<'input> = dyn Fn(Option<&str>, &str) -> core::result::Result<Option<&'input str>, ResolutionError>
  + 'input;

impl Default for ParsingOptions<'_> {
  fn default() -> Self {
    ParsingOptions {
      dtd: crate::Dtd::Deny,
      nodes_limit: u32::MAX,
      entity_resolver: None,
    }
  }
}

impl fmt::Debug for ParsingOptions<'_> {
  fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
    let entity_resolver = match self.entity_resolver {
      Some(_) => "Some(..)",
      None => "None",
    };

    fmt
      .debug_struct("ParsingOptions")
      .field("dtd", &self.dtd)
      .field("nodes_limit", &self.nodes_limit)
      .field("entity_resolver", &entity_resolver)
      .finish()
  }
}

struct TempAttributeData<'input> {
  prefix: &'input str,
  local: &'input str,
  value: StringStorage<'input>,
  range: Range<u64>,
  #[cfg(feature = "positions")]
  qname_len: u16,
  #[cfg(feature = "positions")]
  eq_len: u8,
}

impl<'input> Document<'input> {
  #[inline]
  pub fn parse(text: &'input str) -> Result<Self> {
    Self::parse_with_options(text, ParsingOptions::default())
  }

  #[inline]
  pub fn parse_with_options(text: &'input str, opt: ParsingOptions<'input>) -> Result<Self> {
    parse(text, opt)
  }
}

type Entities<'input> = alloc::collections::BTreeMap<&'input str, StrSpan<'input>>;

#[derive(Clone, Copy)]
struct TagNameSpan<'input> {
  prefix: &'input str,
  name: &'input str,
  pos: u64,
  prefix_pos: u64,
}

impl<'input> TagNameSpan<'input> {
  #[inline]
  fn new_null() -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      prefix: "",
      name: "",
      pos: 0,
      prefix_pos: 0,
    })
  }
}

const MAX_ENTITY_DEPTH: u8 = 10;
const MAX_ENTITY_REFERENCES: u16 = 255;

#[derive(Default)]
struct LoopDetector {
  depth: u8,

  references: u16,
}

impl LoopDetector {
  #[inline]
  fn inc_depth(&mut self, stream: &Stream) -> Result<()> {
    match self.depth {
      ..MAX_ENTITY_DEPTH => {
        self.depth = u8::wrapping_add(self.depth, 1);
        Ok(())
      }
      MAX_ENTITY_DEPTH.. => {
        let Ok(gen_text_pos) = stream.gen_text_pos();

        Err(Error::EntityReferenceLoop(gen_text_pos))
      }
    }
  }

  #[inline]
  fn dec_depth(&mut self) -> core::result::Result<(), console_core_never::Never> {
    match self.depth {
      1.. => {
        self.depth = u8::wrapping_sub(self.depth, 1);
      }
      0 => {}
    }

    match self.depth {
      0 => {
        self.references = 0;
      }
      1.. => {}
    }

    Ok(())
  }

  #[inline]
  fn inc_references(&mut self, stream: &Stream) -> Result<()> {
    match self.depth {
      0 => Ok(()),
      1.. => {
        match self.references {
          MAX_ENTITY_REFERENCES.. => {
            let Ok(gen_text_pos) = stream.gen_text_pos();
            return Err(Error::EntityReferenceLoop(gen_text_pos));
          }
          ..MAX_ENTITY_REFERENCES => {}
        }

        self.references = self.references.saturating_add(1);
        Ok(())
      }
    }
  }
}

struct Context<'input> {
  opt: ParsingOptions<'input>,
  namespace_start_idx: u64,
  current_attributes: Vec<TempAttributeData<'input>>,
  awaiting_subtree: Vec<NodeId>,
  parent_prefixes: Vec<&'input str>,
  entities: Entities<'input>,
  after_text: Vec<Cow<'input, str>>,
  parent_id: NodeId,
  tag_name: TagNameSpan<'input>,
  loop_detector: LoopDetector,
  doc: Document<'input>,
}

impl<'input> Context<'input> {
  fn append_node(&mut self, kind: NodeKind<'input>, range: Range<u64>) -> Result<NodeId> {
    let Ok(length) = self.doc.nodes.len();

    match length.cmp(&u64::from(self.opt.nodes_limit)) {
      Ordering::Equal | Ordering::Greater => return Err(Error::NodesLimitReached),
      Ordering::Less => {}
    }

    #[cfg(not(feature = "positions"))]
    let _range = range;

    enum Subtree {
      Element,
      Leaf,
    }

    let subtree = match kind {
      NodeKind::Element { .. } => Subtree::Element,
      NodeKind::Root | NodeKind::PI(_) | NodeKind::Comment(_) | NodeKind::Text(_) => {
        Subtree::Leaf
      }
    };
    let new_child_id = self.doc.nodes.push(NodeData {
      parent: Some(self.parent_id),
      prev_sibling: None,
      next_subtree: None,
      last_child: None,
      kind,
      #[cfg(feature = "positions")]
      range,
    })?;
    let Ok(found_parent) = self.doc.nodes.get(self.parent_id);
    let parent = found_parent.ok_or(Error::NoRootNode)?;
    let last_child_id = parent.last_child;
    let Ok(found_child) = self.doc.nodes.get_mut(new_child_id);

    let child = found_child.ok_or(Error::NoRootNode)?;
    child.prev_sibling = last_child_id;

    let Ok(found_parent_mut) = self.doc.nodes.get_mut(self.parent_id);

    let parent_mut = found_parent_mut.ok_or(Error::NoRootNode)?;
    parent_mut.last_child = Some(new_child_id);

    for id in &self.awaiting_subtree {
      let Ok(found_node) = self.doc.nodes.get_mut(*id);

      let node = found_node.ok_or(Error::NoRootNode)?;
      node.next_subtree = Some(new_child_id);
    }

    self.awaiting_subtree.clear();

    match subtree {
      Subtree::Element => {}
      Subtree::Leaf => self.awaiting_subtree.push(new_child_id),
    }

    Ok(new_child_id)
  }

  fn append_text(&mut self, text: Cow<'input, str>, range: Range<u64>) -> Result<()> {
    match self.after_text.as_slice() {
      [] => {
        let text_4 = match &text {
          Cow::Borrowed(text_2) => StringStorage::Borrowed(text_2),
          Cow::Owned(text_3) => {
            let Ok(string_storage) = StringStorage::new_owned(text_3.as_str());

            string_storage
          }
        };

        self.append_node(NodeKind::Text(text_4), range)?;
      }
      [_, ..] => {}
    }

    self.after_text.push(text);
    Ok(())
  }

  #[cold]
  #[inline(never)]
  fn merge_text(&mut self) -> Result<()> {
    let Ok(node) = self.doc.nodes.last_mut();

    let text = match &mut node.kind {
      NodeKind::Text(text) => text,
      NodeKind::Root | NodeKind::Element { .. } | NodeKind::Comment(_) | NodeKind::PI(_) => {
        return Err(Error::MissingTextNode);
      }
    };

    let Ok(string_storage) = StringStorage::new_owned(&self.after_text.join(""));
    *text = string_storage;

    Ok(())
  }

  #[inline]
  fn reset_after_text(&mut self) -> Result<()> {
    match self.after_text.as_slice() {
      [] => return Ok(()),
      [_] => {}
      [_, _, ..] => {
        self.merge_text()?;
      }
    }

    self.after_text.clear();

    Ok(())
  }
}

fn parse<'input>(text: &'input str, opt: ParsingOptions<'input>) -> Result<Document<'input>> {
  let Ok(nodes_capacity) =
    console_core_number_conversion::fitted::<_, u64>(memchr_iter(b'<', text.as_bytes()).count());
  let Ok(attributes_capacity) =
    console_core_number_conversion::fitted::<_, u64>(memchr_iter(b'=', text.as_bytes()).count());
  let Ok(attribute_slots) = console_core_number_conversion::index(attributes_capacity);

  #[cfg(feature = "positions")]
  let Ok(text_length) = console_core_number_conversion::fitted::<_, u64>(text.len());
  let root = NodeData {
    parent: None,
    prev_sibling: None,
    next_subtree: None,
    last_child: None,
    kind: NodeKind::Root,
    #[cfg(feature = "positions")]
    range: 0..text_length,
  };
  let Ok(nodes) = crate::NodeList::new(root, nodes_capacity);
  let mut initial_doc = Document {
    text,
    nodes,
    attributes: Vec::with_capacity(attribute_slots.min(4_096)),
    namespaces: Namespaces::default(),
  };

  initial_doc
    .namespaces
    .push_ns(Some(NS_XML_PREFIX), StringStorage::Borrowed(NS_XML_URI))?;

  let dtd = opt.dtd;

  let mut ctx = Context {
    opt,
    namespace_start_idx: 1,
    current_attributes: Vec::with_capacity(16),
    entities: Entities::new(),
    awaiting_subtree: Vec::new(),
    parent_prefixes: vec![""],
    after_text: Vec::with_capacity(1),
    parent_id: ({
      let Ok(explicit_value) =
        core::result::Result::<_, console_core_never::Never>::Ok(NodeId::ROOT);
      explicit_value
    }),
    tag_name: ({
      let Ok(explicit_value) = TagNameSpan::new_null();
      explicit_value
    }),
    loop_detector: LoopDetector::default(),
    doc: initial_doc,
  };

  tokenizer::parse(text, dtd, &mut ctx)?;

  let mut doc = ctx.doc;

  let Ok(doc_root) = doc.root();
  let Ok(mut children) = doc_root.children();

  match children.find(|n| {
    let Ok(is_element) = n.is_element();

    is_element == crate::NodeMatch::Matches
  }) {
    None => {
      return Err(Error::NoRootNode);
    }
    Some(_root_element) => {}
  }

  match ctx.parent_prefixes.as_slice() {
    [_, _, ..] => {
      return Err(Error::UnclosedRootNode);
    }
    [] | [_] => {}
  }

  let Ok(()) = doc.nodes.shrink_to_fit();
  doc.attributes.shrink_to_fit();
  let Ok(()) = doc.namespaces.shrink_to_fit();

  Ok(doc)
}

impl<'input> tokenizer::XmlEvents<'input> for Context<'input> {
  #[inline(always)]
  fn token(&mut self, token: tokenizer::Token<'input>) -> Result<()> {
    match token {
      tokenizer::Token::ProcessingInstruction(target, value, range) => {
        self.reset_after_text()?;
        let pi = NodeKind::PI(PI { target, value });
        self.append_node(pi, range)?;
      }
      tokenizer::Token::Comment(text, range) => {
        self.reset_after_text()?;
        self.append_node(NodeKind::Comment(StringStorage::Borrowed(text)), range)?;
      }
      tokenizer::Token::EntityDeclaration(name, definition) => {
        let _first_definition = self.entities.entry(name).or_insert(definition);
      }
      tokenizer::Token::ElementStart(prefix, local, start) => {
        self.reset_after_text()?;

        match prefix {
          XMLNS => {
            let Ok(pos) = self.doc.text_pos_at(u64::wrapping_add(
              {
                let Ok(converted) = console_core_number_conversion::fitted::<_, u64>(start);
                converted
              },
              {
                let Ok(converted) = console_core_number_conversion::fitted::<_, u64>(1);
                converted
              },
            ));
            return Err(Error::InvalidElementNamePrefix(pos));
          }
          _ => {}
        }

        self.tag_name = TagNameSpan {
          prefix,
          name: local,
          pos: start,
          prefix_pos: u64::wrapping_add(
            {
              let Ok(converted) = console_core_number_conversion::fitted::<_, u64>(start);
              converted
            },
            {
              let Ok(converted) = console_core_number_conversion::fitted::<_, u64>(1);
              converted
            },
          ),
        };
      }
      tokenizer::Token::Attribute(range, qname_len, eq_len, prefix, local, value) => {
        process_attribute(
          range,
          qname_len,
          eq_len,
          QualifiedName { prefix, local },
          value,
          self,
        )?;
      }
      tokenizer::Token::ElementEnd(end, range) => {
        self.reset_after_text()?;
        process_element(end, range, self)?;
      }
      tokenizer::Token::Text(text, range) => {
        process_text(text, range, self)?;
      }
      tokenizer::Token::Cdata(text, range) => {
        process_cdata(text, range, self)?;
      }
    }

    Ok(())
  }

  fn resolve_entity(
    &mut self,
    pub_id: Option<&str>,
    uri: &str,
  ) -> core::result::Result<Option<&'input str>, ResolutionError> {
    match &mut self.opt.entity_resolver {
      Some(entity_resolver) => entity_resolver(pub_id, uri),
      None => Ok(None),
    }
  }
}

struct QualifiedName<'a> {
  prefix: &'a str,
  local: &'a str,
}

fn process_attribute<'input>(
  range: Range<u64>,
  qname_len: u16,
  eq_len: u8,
  name: QualifiedName<'input>,
  source: StrSpan<'input>,
  ctx: &mut Context<'input>,
) -> Result<()> {
  let QualifiedName { prefix, local } = name;
  let value = normalize_attribute(source, ctx)?;

  match (prefix, local) {
    (XMLNS, _) => {
      let Ok(uri) = value.as_str();

      match (local, uri) {
        (_, NS_XMLNS_URI) => {
          let Ok(pos) = ctx.doc.text_pos_at(range.start);
          return Err(Error::UnexpectedXmlnsUri(pos));
        }
        (NS_XML_PREFIX, NS_XML_URI) => {}
        (NS_XML_PREFIX, _) => {
          let Ok(pos) = ctx.doc.text_pos_at(range.start);
          return Err(Error::InvalidXmlPrefixUri(pos));
        }
        (_, NS_XML_URI) => {
          let Ok(pos) = ctx.doc.text_pos_at(range.start);
          return Err(Error::UnexpectedXmlUri(pos));
        }
        (_, _) => {}
      }

      let Ok(exists) = ctx
        .doc
        .namespaces
        .exists(ctx.namespace_start_idx, Some(local));

      match exists {
        crate::Presence::Present => {
          let Ok(pos) = ctx.doc.text_pos_at(range.start);
          return Err(Error::DuplicatedNamespace(local.to_string(), pos));
        }
        crate::Presence::Absent => {}
      }

      match uri {
        NS_XML_URI => {}
        _ => ctx.doc.namespaces.push_ns(Some(local), value)?,
      }
    }
    (_, XMLNS) => {
      let Ok(uri) = value.as_str();

      match uri {
        NS_XML_URI => {
          let Ok(pos) = ctx.doc.text_pos_at(range.start);
          return Err(Error::UnexpectedXmlUri(pos));
        }
        NS_XMLNS_URI => {
          let Ok(pos) = ctx.doc.text_pos_at(range.start);
          return Err(Error::UnexpectedXmlnsUri(pos));
        }
        _ => ctx.doc.namespaces.push_ns(None, value)?,
      }
    }
    (_, _) => {
      #[cfg(not(feature = "positions"))]
      let _ = (qname_len, eq_len);

      ctx.current_attributes.push(TempAttributeData {
        prefix,
        local,
        value,
        range,
        #[cfg(feature = "positions")]
        qname_len,
        #[cfg(feature = "positions")]
        eq_len,
      });
    }
  }

  Ok(())
}

fn process_element<'input>(
  end_token: tokenizer::ElementEnd<'input>,
  token_range: Range<u64>,
  ctx: &mut Context<'input>,
) -> Result<()> {
  match ctx.tag_name.name {
    "" => {
      let Ok(text_pos_at) = ctx.doc.text_pos_at(token_range.start);
      return Err(Error::UnexpectedEntityCloseTag(text_pos_at));
    }
    _ => {}
  }

  let namespaces = ctx.resolve_namespaces()?;
  let Ok(length) =
    console_core_number_conversion::fitted::<_, u64>(ctx.doc.namespaces.tree_order.len());
  ctx.namespace_start_idx = length;

  let attributes = resolve_attributes(namespaces, ctx)?;

  match end_token {
    tokenizer::ElementEnd::Empty => {
      let tag_ns_idx = get_ns_idx_by_prefix(
        namespaces,
        ctx.tag_name.prefix_pos,
        ctx.tag_name.prefix,
        &ctx.doc,
      )?;
      let new_element_id = ctx.append_node(
        NodeKind::Element {
          tag_name: ExpandedNameIndexed {
            namespace_idx: tag_ns_idx,
            local_name: ctx.tag_name.name,
          },
          attributes,
          namespaces,
        },
        ctx.tag_name.pos..token_range.end,
      )?;
      ctx.awaiting_subtree.push(new_element_id);
    }
    tokenizer::ElementEnd::Close(prefix, local) => {
      close_element(QualifiedName { prefix, local }, token_range, ctx)?;
    }
    tokenizer::ElementEnd::Open => {
      let tag_ns_idx = get_ns_idx_by_prefix(
        namespaces,
        ctx.tag_name.prefix_pos,
        ctx.tag_name.prefix,
        &ctx.doc,
      )?;
      let parent_id = ctx.append_node(
        NodeKind::Element {
          tag_name: ExpandedNameIndexed {
            namespace_idx: tag_ns_idx,
            local_name: ctx.tag_name.name,
          },
          attributes,
          namespaces,
        },
        ctx.tag_name.pos..token_range.end,
      )?;
      ctx.parent_id = parent_id;
      ctx.parent_prefixes.push(ctx.tag_name.prefix);
    }
  }

  Ok(())
}

fn close_element(
  closing: QualifiedName<'_>,
  token_range: Range<u64>,
  ctx: &mut Context<'_>,
) -> Result<()> {
  let Ok(parent) = ctx.doc.nodes.get_mut(ctx.parent_id);
  let parent_node = parent.ok_or(Error::NoRootNode)?;
  let parent_prefix = ctx
    .parent_prefixes
    .last()
    .copied()
    .ok_or(Error::MissingParentPrefix)?;

  #[cfg(feature = "positions")]
  {
    parent_node.range.end = token_range.end;
  }

  match &parent_node.kind {
    NodeKind::Element { tag_name, .. } => {
      let opening = QualifiedName {
        prefix: parent_prefix,
        local: tag_name.local_name,
      };

      match crate::NodeMatch::from(
        opening.prefix == closing.prefix && opening.local == closing.local,
      ) {
        crate::NodeMatch::Different => {
          let Ok(expected) = gen_qname_string(opening);
          let Ok(found) = gen_qname_string(closing);
          let Ok(position) = ctx.doc.text_pos_at(token_range.start);

          return Err(Error::UnexpectedCloseTag(expected, found, position));
        }
        crate::NodeMatch::Matches => {}
      }
    }
    NodeKind::Root | NodeKind::PI(_) | NodeKind::Text(_) | NodeKind::Comment(_) => {}
  }

  ctx.awaiting_subtree.push(ctx.parent_id);

  match parent_node.parent {
    Some(id) => {
      ctx.parent_id = id;
      ctx.parent_prefixes.pop();
      ctx
        .parent_prefixes
        .last()
        .ok_or(Error::MissingParentPrefix)?;

      Ok(())
    }
    None => {
      let Ok(position) = ctx.doc.text_pos_at(token_range.start);

      Err(Error::UnexpectedEntityCloseTag(position))
    }
  }
}

impl Context<'_> {
  fn resolve_namespaces(&mut self) -> Result<ShortRange> {
    let Ok(parent) = self.doc.nodes.get(self.parent_id);

    match parent.map(|node| &node.kind) {
      Some(NodeKind::Element { namespaces, .. }) => {
        let parent_ns = *namespaces;

        let Ok(namespace_end) =
          console_core_number_conversion::fitted::<_, u64>(self.doc.namespaces.tree_order.len());

        match self.namespace_start_idx.cmp(&namespace_end) {
          Ordering::Equal => {
            return Ok(parent_ns);
          }
          Ordering::Less | Ordering::Greater => {}
        }

        let Ok(urange) = parent_ns.to_urange();

        for i in urange {
          let Ok(position) = console_core_number_conversion::index(i);
          let namespace_idx = self
            .doc
            .namespaces
            .tree_order
            .get(position)
            .copied()
            .ok_or(Error::InvalidNamespaceIndex)?;
          let Ok(namespace) = self.doc.namespaces.get(namespace_idx);
          let namespace_2 = namespace.ok_or(Error::InvalidNamespaceIndex)?;

          let Ok(exists) = self
            .doc
            .namespaces
            .exists(self.namespace_start_idx, namespace_2.name);

          match exists {
            crate::Presence::Absent => {
              self.doc.namespaces.push_ref(i)?;
            }
            crate::Presence::Present => {}
          }
        }
      }
      Some(NodeKind::Root | NodeKind::PI(_) | NodeKind::Text(_) | NodeKind::Comment(_))
      | None => {}
    }

    let Ok(end) =
      console_core_number_conversion::fitted::<_, u64>(self.doc.namespaces.tree_order.len());
    let short_range = ShortRange::try_from(self.namespace_start_idx..end)?;

    Ok(short_range)
  }
}

fn resolve_attributes(namespaces: ShortRange, ctx: &mut Context) -> Result<ShortRange> {
  match ctx.current_attributes.as_slice() {
    [] => {
      let Ok(short_range) = ShortRange::new(0..0);
      return Ok(short_range);
    }
    [_, ..] => {}
  }

  let Ok(length) = console_core_number_conversion::fitted::<_, u64>(ctx.doc.attributes.len());
  let Ok(ctx_length) =
    console_core_number_conversion::fitted::<_, u64>(ctx.current_attributes.len());

  match length.wrapping_add(ctx_length).cmp(&u64::from(u32::MAX)) {
    Ordering::Equal | Ordering::Greater => {
      return Err(Error::AttributesLimitReached);
    }
    Ordering::Less => {}
  }

  let Ok(start_idx) = console_core_number_conversion::fitted::<_, u64>(ctx.doc.attributes.len());

  for attr in ctx.current_attributes.drain(..) {
    let namespace_idx = match attr.prefix {
      NS_XML_PREFIX => Some(NamespaceIdx(0)),
      "" => None,
      _ => get_ns_idx_by_prefix(namespaces, attr.range.start, attr.prefix, &ctx.doc)?,
    };

    let attr_name = ExpandedNameIndexed {
      namespace_idx,
      local_name: attr.local,
    };

    let Ok(range) = console_core_number_conversion::indices_from(start_idx..);
    let prior_attributes = ctx
      .doc
      .attributes
      .get(range)
      .ok_or(Error::InvalidAttributeIndex)?;

    match prior_attributes.iter().find(|candidate| {
      let Ok(expanded_name) = candidate.name.as_expanded_name(&ctx.doc);
      let Ok(attr_name_expanded_name) = attr_name.as_expanded_name(&ctx.doc);

      expanded_name == attr_name_expanded_name
    }) {
      Some(_duplicate) => {
        let Ok(pos) = ctx.doc.text_pos_at(attr.range.start);
        return Err(Error::DuplicatedAttribute(attr.local.to_string(), pos));
      }
      None => {}
    }

    ctx.doc.attributes.push(AttributeData {
      name: attr_name,
      value: attr.value,
      #[cfg(feature = "positions")]
      range: attr.range,
      #[cfg(feature = "positions")]
      qname_len: attr.qname_len,
      #[cfg(feature = "positions")]
      eq_len: attr.eq_len,
    });
  }

  let Ok(end) = console_core_number_conversion::fitted::<_, u64>(ctx.doc.attributes.len());

  ShortRange::try_from(start_idx..end)
}

enum NextTextByte {
  Raw,
  Normalize,
}

fn push_text_character(
  character: char,
  buffer: &mut TextBuffer,
  next_byte: &mut NextTextByte,
  depth: u8,
  end: crate::End,
) -> core::result::Result<(), console_core_never::Never> {
  let Ok(bytes) = CharToBytes::new(character);

  for byte in bytes {
    match depth {
      0 => {
        let Ok(()) = buffer.push_raw(byte);
        *next_byte = NextTextByte::Raw;
      }
      1.. => {
        let Ok(()) = buffer.push_from_text(byte, end);
      }
    }
  }

  Ok(())
}

fn process_text<'input>(
  text: &'input str,
  range: Range<u64>,
  ctx: &mut Context<'input>,
) -> Result<()> {
  match memchr2(b'&', b'\r', text.as_bytes()) {
    None => {
      ctx.append_text(Cow::Borrowed(text), range)?;
      return Ok(());
    }
    Some(_normalization_point) => {}
  }

  let Ok(mut text_buffer) = TextBuffer::new();
  let mut next_byte = NextTextByte::Normalize;
  let mut stream = Stream::from_substr(ctx.doc.text, range.clone())?;
  let Ok(budget) = stream.as_bytes();

  for _step in budget {
    let Ok(end) = stream.at_end();

    match end {
      crate::End::Reached => break,
      crate::End::Pending => {}
    }

    let explicit_value_1 = parse_next_chunk(&mut stream, &ctx.entities)?;

    match explicit_value_1 {
      NextChunk::Byte(c) => match next_byte {
        NextTextByte::Raw => {
          let Ok(()) = text_buffer.push_raw(c);
          next_byte = NextTextByte::Normalize;
        }
        NextTextByte::Normalize => {
          let Ok(at_end) = stream.at_end();
          let Ok(()) = text_buffer.push_from_text(c, at_end);
        }
      },
      NextChunk::Char(c) => {
        let Ok(at_end) = stream.at_end();
        let Ok(()) = push_text_character(
          c,
          &mut text_buffer,
          &mut next_byte,
          ctx.loop_detector.depth,
          at_end,
        );
      }
      NextChunk::Text(fragment) => {
        next_byte = NextTextByte::Normalize;

        let Ok(is_empty) = text_buffer.is_empty();

        match is_empty {
          crate::Emptiness::NonEmpty => {
            let text_2 = text_buffer.finish()?;
            ctx.append_text(Cow::Owned(text_2), range.clone())?;
          }
          crate::Emptiness::Empty => {}
        }

        ctx.loop_detector.inc_references(&stream)?;
        ctx.loop_detector.inc_depth(&stream)?;

        let Ok(fragment_range) = fragment.range();
        let text_3 = match fragment_range.start {
          0 => {
            let Ok(explicit_value) = fragment.as_str();
            explicit_value
          }
          1.. => ctx.doc.text,
        };

        let mut stream_2 = Stream::from_substr(text_3, {
          let Ok(explicit_value) = fragment.range();
          explicit_value
        })?;
        let prev_tag_name = ctx.tag_name;
        let Ok(tag_name_span) = TagNameSpan::new_null();
        ctx.tag_name = tag_name_span;
        tokenizer::parse_content(&mut stream_2, ctx)?;
        ctx.tag_name = prev_tag_name;
        let Ok(()) = text_buffer.clear();

        let Ok(()) = ctx.loop_detector.dec_depth();
      }
    }
  }

  let Ok(is_empty) = text_buffer.is_empty();

  match is_empty {
    crate::Emptiness::NonEmpty => {
      let text_4 = text_buffer.finish()?;
      ctx.append_text(Cow::Owned(text_4), range)?;
    }
    crate::Emptiness::Empty => {}
  }

  Ok(())
}

fn process_cdata<'input>(
  text: &'input str,
  range: Range<u64>,
  ctx: &mut Context<'input>,
) -> Result<()> {
  match text.find('\r') {
    None => return ctx.append_text(Cow::Borrowed(text), range),
    Some(_carriage_return) => {}
  }

  enum Previous {
    CarriageReturn,
    Other,
  }

  let mut previous = Previous::Other;
  let mut normalized = String::with_capacity(text.len());

  for ch in text.chars() {
    match ch {
      '\r' => {
        normalized.push('\n');
        previous = Previous::CarriageReturn;
      }
      '\n' => {
        match previous {
          Previous::CarriageReturn => {}
          Previous::Other => normalized.push('\n'),
        }

        previous = Previous::Other;
      }
      other => {
        normalized.push(other);
        previous = Previous::Other;
      }
    }
  }

  ctx.append_text(Cow::Owned(normalized), range)
}

enum NextChunk<'a> {
  Byte(u8),
  Char(char),
  Text(StrSpan<'a>),
}

fn parse_next_chunk<'a>(
  stream: &mut Stream<'a>,
  entities: &Entities<'a>,
) -> Result<NextChunk<'a>> {
  let c = stream.curr_byte()?;

  match c {
    b'&' => {
      let Ok(stream_pos) = stream.pos();
      let Ok(start) = console_core_number_conversion::fitted::<_, u64>(stream_pos);
      let explicit_value_2 = stream.consume_reference()?;

      match explicit_value_2 {
        Some(Reference::Char(ch)) => Ok(NextChunk::Char(ch)),
        Some(Reference::Entity(name)) => entities
          .get(name)
          .copied()
          .map(NextChunk::Text)
          .ok_or_else(|| {
            let Ok(pos) = stream.gen_text_pos_from(start);
            Error::UnknownEntityReference(String::from(name), pos)
          }),
        None => {
          let Ok(pos) = stream.gen_text_pos_from(start);
          Err(Error::MalformedEntityReference(pos))
        }
      }
    }
    _ => {
      let Ok(()) = stream.advance(1);
      Ok(NextChunk::Byte(c))
    }
  }
}

fn normalize_attribute<'input>(
  text: StrSpan<'input>,
  ctx: &mut Context<'input>,
) -> Result<StringStorage<'input>> {
  let Ok(value) = text.as_str();
  let bytes = value.as_bytes();

  match memchr2(b'&', b'\t', bytes).or_else(|| memchr2(b'\n', b'\r', bytes)) {
    Some(_normalization_point) => {
      let Ok(mut text_buffer) = TextBuffer::new();
      _normalize_attribute(text, &mut text_buffer, ctx)?;
      let text_2 = text_buffer.finish()?;
      let Ok(text_3) = StringStorage::new_owned(&text_2);

      Ok(text_3)
    }
    None => Ok(StringStorage::Borrowed(value)),
  }
}

fn push_attribute_character(
  character: char,
  buffer: &mut TextBuffer,
  depth: u8,
  stream: &Stream<'_>,
  start: u64,
) -> Result<()> {
  match (depth, character) {
    (1.., '<') => {
      let Ok(position) = stream.gen_text_pos_from(start);

      return Err(Error::InvalidAttributeValue(position));
    }
    (0, _) | (1.., _) => {}
  }

  let Ok(bytes) = CharToBytes::new(character);

  for byte in bytes {
    match depth {
      0 => {
        let Ok(()) = buffer.push_raw(byte);
      }
      1.. => {
        let Ok(()) = buffer.push_from_attr(byte, None);
      }
    }
  }

  Ok(())
}

fn _normalize_attribute<'input>(
  text: StrSpan<'input>,
  buffer: &mut TextBuffer,
  ctx: &mut Context<'input>,
) -> Result<()> {
  enum Expansion {
    Outer,
    Entity,
  }

  struct Frame<'input> {
    stream: Stream<'input>,
    expansion: Expansion,
  }

  let Ok(fragment) = text.as_str();
  let Ok(length) = console_core_number_conversion::fitted::<_, u64>(fragment.len());
  let Ok(source_length) = console_core_number_conversion::fitted::<_, u64>(ctx.doc.text.len());
  let maximum_expansions = u64::from(MAX_ENTITY_REFERENCES).saturating_add(2);
  let budget = length
    .max(1)
    .saturating_mul(source_length.saturating_add(2))
    .saturating_mul(maximum_expansions);
  let Ok(range) = text.range();
  let stream = Stream::from_substr(ctx.doc.text, range)?;
  let mut frames = vec![Frame {
    stream,
    expansion: Expansion::Outer,
  }];

  for _step in 0..budget {
    let mut frame = match frames.pop() {
      Some(frame) => frame,
      None => return Ok(()),
    };
    let Ok(end) = frame.stream.at_end();

    match end {
      crate::End::Reached => {
        match frame.expansion {
          Expansion::Outer => {}
          Expansion::Entity => {
            let Ok(()) = ctx.loop_detector.dec_depth();
          }
        }

        continue;
      }
      crate::End::Pending => {}
    }

    let byte = frame.stream.curr_byte()?;

    match byte {
      b'&' => {}
      other => {
        let Ok(()) = frame.stream.advance(1);
        let Ok(next) = frame.stream.peek();
        let Ok(()) = buffer.push_from_attr(other, next);

        frames.push(frame);

        continue;
      }
    }

    let Ok(start) = frame.stream.pos();
    let reference = frame.stream.consume_reference()?;

    match reference {
      Some(Reference::Char(ch)) => {
        push_attribute_character(ch, buffer, ctx.loop_detector.depth, &frame.stream, start)?;
        frames.push(frame);
      }
      Some(Reference::Entity(name)) => {
        let value = match ctx.entities.get(name) {
          Some(value) => *value,
          None => {
            let Ok(position) = frame.stream.gen_text_pos_from(start);

            return Err(Error::UnknownEntityReference(String::from(name), position));
          }
        };

        ctx.loop_detector.inc_references(&frame.stream)?;
        ctx.loop_detector.inc_depth(&frame.stream)?;

        let Ok(range_2) = value.range();
        let stream_2 = Stream::from_substr(ctx.doc.text, range_2)?;

        frames.push(frame);
        frames.push(Frame {
          stream: stream_2,
          expansion: Expansion::Entity,
        });
      }
      None => {
        let Ok(position) = frame.stream.gen_text_pos_from(start);

        return Err(Error::MalformedEntityReference(position));
      }
    }
  }

  let Ok(range_3) = text.range();
  let Ok(source) = Stream::new(ctx.doc.text);
  let Ok(position) = source.gen_text_pos_from(range_3.start);

  Err(Error::EntityReferenceLoop(position))
}

fn get_ns_idx_by_prefix(
  namespaces: ShortRange,
  prefix_pos: u64,
  prefix: &str,
  doc: &Document<'_>,
) -> Result<Option<NamespaceIdx>> {
  let prefix_opt = match prefix {
    "" => None,
    _ => Some(prefix),
  };

  let Ok(range) = namespaces.to_urange();
  let Ok(range_2) = console_core_number_conversion::indices(range);
  let namespace_indices = doc
    .namespaces
    .tree_order
    .get(range_2)
    .ok_or(Error::InvalidNamespaceIndex)?;
  let idx = namespace_indices.iter().find(|idx| {
    let Ok(namespace) = doc.namespaces.get(**idx);

    namespace.is_some_and(|value| value.name == prefix_opt)
  });

  match idx {
    Some(idx_2) => Ok(Some(*idx_2)),
    None => match prefix {
      "" => Ok(None),
      _ => {
        let Ok(pos) = doc.text_pos_at(prefix_pos);
        Err(Error::UnknownNamespace(prefix.to_string(), pos))
      }
    },
  }
}

fn gen_qname_string(
  name: QualifiedName<'_>,
) -> core::result::Result<String, console_core_never::Never> {
  let QualifiedName { prefix, local } = name;

  Ok(match prefix {
    "" => local.to_string(),
    _ => {
      alloc::format!("{}:{}", prefix, local)
    }
  })
}

struct CharToBytes {
  bytes: core::iter::Take<core::array::IntoIter<u8, 4>>,
}

impl CharToBytes {
  fn new(ch: char) -> core::result::Result<Self, console_core_never::Never> {
    let mut buffer = [0u8; 4];
    let text = ch.encode_utf8(&mut buffer);
    let Ok(length) = console_core_number_conversion::fitted::<_, u64>(text.len());
    let Ok(limit) = console_core_number_conversion::index(length);

    Ok(Self {
      bytes: buffer.into_iter().take(limit),
    })
  }
}

impl Iterator for CharToBytes {
  type Item = u8;

  fn next(&mut self) -> Option<Self::Item> {
    self.bytes.next()
  }
}

struct TextBuffer {
  buffer: Vec<u8>,
}

impl TextBuffer {
  #[inline]
  fn new() -> core::result::Result<Self, console_core_never::Never> {
    Ok(TextBuffer {
      buffer: Vec::with_capacity(32),
    })
  }

  #[inline]
  fn push_raw(&mut self, c: u8) -> core::result::Result<(), console_core_never::Never> {
    self.buffer.push(c);

    Ok(())
  }

  fn push_from_attr(
    &mut self,
    mut current: u8,
    next: Option<u8>,
  ) -> core::result::Result<(), console_core_never::Never> {
    match (current, next) {
      (b'\r', Some(b'\n')) => {
        return Ok(());
      }
      (_, _) => {}
    }

    current = match current {
      b'\n' | b'\r' | b'\t' => b' ',
      _ => current,
    };

    self.buffer.push(current);

    Ok(())
  }

  fn push_from_text(
    &mut self,
    byte: u8,
    ending: crate::End,
  ) -> core::result::Result<(), console_core_never::Never> {
    enum Previous {
      CarriageReturn,
      Other,
    }

    let previous = match self.buffer.last_mut() {
      Some(last) => match *last {
        b'\r' => {
          *last = b'\n';

          Previous::CarriageReturn
        }
        _other => Previous::Other,
      },
      None => Previous::Other,
    };

    match byte {
      b'\n' => match previous {
        Previous::CarriageReturn => {}
        Previous::Other => self.buffer.push(b'\n'),
      },
      b'\r' => match ending {
        crate::End::Reached => self.buffer.push(b'\n'),
        crate::End::Pending => self.buffer.push(b'\r'),
      },
      other => self.buffer.push(other),
    }

    Ok(())
  }

  #[inline]
  fn clear(&mut self) -> core::result::Result<(), console_core_never::Never> {
    self.buffer.clear();

    Ok(())
  }

  #[inline]
  fn is_empty(&self) -> core::result::Result<crate::Emptiness, console_core_never::Never> {
    Ok(match self.buffer.as_slice() {
      [] => crate::Emptiness::Empty,
      [_, ..] => crate::Emptiness::NonEmpty,
    })
  }

  fn finish(&mut self) -> Result<String> {
    String::from_utf8(take(&mut self.buffer))
      .map_err(|_invalid_utf8| Error::InvalidNormalizedText)
  }
}
