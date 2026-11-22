//! HTML5 recovery and target serialization share one flat, non-recursive tree.

use super::{DompurifyEffectiveConfig, SanitizeFailure, SanitizeOutputSink};
use html5ever::tendril::StrTendril;
use html5ever::tokenizer::{BufferQueue, TokenSink, TokenSinkResult, Tokenizer};
use html5ever::tree_builder::{
    ElementFlags, NodeOrText, QuirksMode, Tracer, TreeBuilder, TreeSink,
};
use html5ever::{Attribute, LocalName, Namespace, QualName, TokenizerResult, ns};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

const NONE: usize = usize::MAX;
const CHUNK_BYTES: usize = 256;
const PARSE_WORK_BATCH: usize = 64;
const WORK_BYTES_PER_UNIT: usize = 256;

#[derive(Clone, Debug)]
struct Handle {
    index: usize,
    name: QualName,
    integration: bool,
    attribute_bytes: usize,
}

impl html5ever::tree_builder::ElemName for &Handle {
    fn ns(&self) -> &Namespace {
        &self.name.ns
    }

    fn local_name(&self) -> &LocalName {
        &self.name.local
    }
}

enum Kind {
    Container,
    Element {
        name: QualName,
        attrs: Vec<Attribute>,
    },
    Text(StrTendril),
    Ignored,
}

struct Node {
    kind: Kind,
    parent: usize,
    first: usize,
    last: usize,
    prev: usize,
    next: usize,
    template: usize,
}

impl Node {
    #[must_use]
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            parent: NONE,
            first: NONE,
            last: NONE,
            prev: NONE,
            next: NONE,
            template: NONE,
        }
    }
}

struct State<E> {
    nodes: Vec<Node>,
    retained: usize,
    error: Option<SanitizeFailure<E>>,
    merged_attribute_names: HashMap<usize, HashSet<QualName>>,
    parse_work: usize,
    total_parse_work: usize,
    parse_work_limit: Option<usize>,
}

struct Builder<'a, S: SanitizeOutputSink> {
    state: RefCell<State<S::Error>>,
    sink: &'a S,
    input_bytes: usize,
}

/// Stops forwarding tokens to the HTML5 tree builder after a sink rejects work.
///
/// `TreeSink` cannot return a `Result`, so a failed callback otherwise leaves the
/// tokenizer running through every remaining token in the chunk. This adapter uses
/// html5ever's public `TokenSink` boundary. Resource admission observes token kinds
/// but does not implement HTML parsing, recovery, or scope rules of its own.
struct StoppingTreeBuilder<'a, S: SanitizeOutputSink> {
    inner: TreeBuilder<Handle, Builder<'a, S>>,
    stopped: std::cell::Cell<bool>,
    complete_token: std::cell::Cell<bool>,
}

struct FormattingWork<'a, 'b, S: SanitizeOutputSink> {
    builder: &'a Builder<'b, S>,
    tag: &'a html5ever::tokenizer::Tag,
    attribute_bytes: usize,
}

impl<S: SanitizeOutputSink> Tracer for FormattingWork<'_, '_, S> {
    type Handle = Handle;

    fn trace_handle(&self, handle: &Handle) {
        match {
            ::core::primitive::bool::then_some(!self.builder.charge_parse_work(1), ())
        } {
            ::core::option::Option::Some(()) => {
                return;
            },
            ::core::option::Option::None => {},
        }

        let attrs = {
            let state = self.builder.state.borrow();
            state
                .nodes
                .get(handle.index)
                .and_then(|node| match &node.kind {
                    Kind::Element { name, attrs }
                        if name.ns == ns!(html) && name.local == self.tag.name =>
                    {
                        Some(attrs.len())
                    }
                    _ => None,
                })
        };

        match attrs {
            Some(attrs) => {
                // equiv_modulo_attr_order clones and sorts both attribute vectors.
                // A squared count bounds that comparison work without duplicating its
                // sorting or the parser's active-formatting state. Live handles can be
                // traced twice (open elements and formatting list), which is conservative.
                let existing = attrs.saturating_add(1);
                let incoming = self.tag.attrs.len().saturating_add(1);
                let comparison_bytes = handle
                    .attribute_bytes
                    .saturating_add(self.attribute_bytes)
                    .div_ceil(WORK_BYTES_PER_UNIT)
                    .max(1);
                let _charge_recorded_in_parser = self.builder.charge_parse_work(
                    existing
                        .saturating_mul(existing)
                        .saturating_add(incoming.saturating_mul(incoming))
                        .saturating_mul(comparison_bytes),
                );
            },
            ::core::option::Option::None => {},
        }
    }
}

impl<S: SanitizeOutputSink> TokenSink for StoppingTreeBuilder<'_, S> {
    type Handle = Handle;

    fn process_token(
        &self,
        token: html5ever::tokenizer::Token,
        line_number: u64,
    ) -> TokenSinkResult<Self::Handle> {
        match {
            ::core::primitive::bool::then_some(self.stopped.get() || self.inner.sink.failed(), ())
        } {
            ::core::option::Option::Some(()) => {
                self.stopped.set(true);
                return TokenSinkResult::Continue;
            },
            ::core::option::Option::None => {},
        }

        match {
            ::core::primitive::bool::then_some(!matches!(token, html5ever::tokenizer::Token::ParseError(_)), ())
        } {
            ::core::option::Option::Some(()) => {
                self.complete_token.set(true);
            },
            ::core::option::Option::None => {},
        }

        if let html5ever::tokenizer::Token::TagToken(tag) = &token
            && tag.kind == html5ever::tokenizer::StartTag
            // Pinned html5ever rules.rs: these are the only start tags that call
            // create_formatting_element_for, whose candidate scan has no callbacks.
            && matches!(tag.name.as_ref(), "a" | "b" | "big" | "code" | "em" | "font"
                | "i" | "s" | "small" | "strike" | "strong" | "tt" | "u" | "nobr")
        {
            self.inner.trace_handles(&FormattingWork {
                builder: &self.inner.sink,
                tag,
                attribute_bytes: tag.attrs.iter().fold(0usize, |bytes, attr| {
                    bytes
                        .saturating_add(attr.name.local.len())
                        .saturating_add(attr.value.len())
                }),
            });

            match {
                ::core::primitive::bool::then_some(!self.inner.sink.flush_parse_work(), ())
            } {
                ::core::option::Option::Some(()) => {
                    self.stopped.set(true);
                    return TokenSinkResult::Continue;
                },
                ::core::option::Option::None => {},
            }
        }

        let result = self.inner.process_token(token, line_number);

        match {
            ::core::primitive::bool::then_some(self.inner.sink.failed(), ())
        } {
            ::core::option::Option::Some(()) => {
                self.stopped.set(true);
            },
            ::core::option::Option::None => {},
        }

        result
    }

    fn end(&self) {
        match {
            ::core::primitive::bool::then_some(!self.stopped.get() && !self.inner.sink.failed(), ())
        } {
            ::core::option::Option::Some(()) => {
                self.inner.end();

                match {
                    ::core::primitive::bool::then_some(self.inner.sink.failed(), ())
                } {
                    ::core::option::Option::Some(()) => {
                        self.stopped.set(true);
                    },
                    ::core::option::Option::None => {},
                }
            },
            ::core::option::Option::None => {},
        }
    }

    fn adjusted_current_node_present_but_not_in_html_namespace(&self) -> bool {
        !self.stopped.get()
            && !self.inner.sink.failed()
            && self
                .inner
                .adjusted_current_node_present_but_not_in_html_namespace()
    }
}

impl<S: SanitizeOutputSink> Builder<'_, S> {
    #[must_use]
    fn failed(&self) -> bool {
        self.state.borrow().error.is_some()
    }

    /// Charge TreeSink callbacks in bounded batches. html5ever's tree builder
    /// performs ancestor and active-formatting scans through these callbacks; the
    /// callbacks are the parser's stable observation point, so this accounting does
    /// not duplicate any HTML parsing logic here.
    #[must_use]
    fn charge_parse_work(&self, additional: usize) -> bool {
        match {
            ::core::primitive::bool::then_some(additional == 0 || self.failed(), ())
        } {
            ::core::option::Option::Some(()) => {
                return !self.failed();
            },
            ::core::option::Option::None => {},
        }

        let charge = {
            let mut state = self.state.borrow_mut();

            let Some(total) = state.total_parse_work.checked_add(additional) else {
                state.error = Some(SanitizeFailure::RejectedInput);
                return false;
            };

            match {
                ::core::primitive::bool::then_some(state.parse_work_limit.is_some_and(|limit| total > limit), ())
            } {
                ::core::option::Option::Some(()) => {
                    state.error = Some(SanitizeFailure::RejectedInput);
                    return false;
                },
                ::core::option::Option::None => {},
            }

            state.total_parse_work = total;
            state.parse_work = state.parse_work.saturating_add(additional);

            match {
                ::core::primitive::bool::then_some(state.parse_work >= PARSE_WORK_BATCH, ())
            } {
                ::core::option::Option::Some(()) => {
                    let charge = state.parse_work;
                    state.parse_work = 0;
                    charge
                },
                ::core::option::Option::None => {
                    0
                },
            }
        };

        if charge != 0
            && let Err(error) = self.sink.charge_parse_work(charge)
        {
            self.state.borrow_mut().error = Some(SanitizeFailure::Output(error));
            return false;
        }

        true
    }

    #[must_use]
    fn flush_parse_work(&self) -> bool {
        match {
            ::core::primitive::bool::then_some(self.failed(), ())
        } {
            ::core::option::Option::Some(()) => {
                return false;
            },
            ::core::option::Option::None => {},
        }

        let charge = {
            let mut state = self.state.borrow_mut();
            let charge = state.parse_work;
            state.parse_work = 0;
            charge
        };

        if charge != 0
            && let Err(error) = self.sink.charge_parse_work(charge)
        {
            self.state.borrow_mut().error = Some(SanitizeFailure::Output(error));
            return false;
        }

        true
    }

    #[must_use]
    fn admit(&self, nodes: usize, bytes: usize) -> bool {
        match {
            ::core::primitive::bool::then_some(self.failed(), ())
        } {
            ::core::option::Option::Some(()) => {
                return false;
            },
            ::core::option::Option::None => {},
        }

        let mut state = self.state.borrow_mut();
        let retained = state.retained.saturating_add(bytes);
        // Input admission bounds tokenizer scratch. This additional bound covers implicit
        // formatting clones and tree bookkeeping before they can amplify retained storage.
        // The tree builder may synthesize table containers and formatting clones. Each
        // retained node costs at least one source byte; allow two complete node/attribute
        // records per input byte plus the document/context/root bookkeeping.
        let record_bytes = 2 * (std::mem::size_of::<Node>() + std::mem::size_of::<Attribute>());
        let ceiling = self
            .input_bytes
            .saturating_add(3)
            .saturating_mul(record_bytes);
        let result = self.sink.checkpoint().and_then(|()| {
            self.sink
                .check_parse_storage(self.input_bytes, nodes, retained)
        });

        match result {
            Err(error) => {
                state.error = Some(SanitizeFailure::Output(error));
                return false;
            },
            ::core::result::Result::Ok(_) => {},
        }

        match {
            ::core::primitive::bool::then_some(retained > ceiling, ())
        } {
            ::core::option::Option::Some(()) => {
                state.error = Some(SanitizeFailure::RejectedInput);
                return false;
            },
            ::core::option::Option::None => {},
        }

        state.retained = retained;
        true
    }

    #[must_use]
    fn allocate(&self, kind: Kind, bytes: usize) -> usize {
        let count = self.state.borrow().nodes.len().saturating_add(1);

        match {
            ::core::primitive::bool::then_some(!self.admit(count, std::mem::size_of::<Node>().saturating_add(bytes)), ())
        } {
            ::core::option::Option::Some(()) => {
                return NONE;
            },
            ::core::option::Option::None => {},
        }

        let mut state = self.state.borrow_mut();

        match {
            ::core::primitive::bool::then_some(state.nodes.try_reserve(1).is_err(), ())
        } {
            ::core::option::Option::Some(()) => {
                state.error = Some(SanitizeFailure::RejectedInput);
                return NONE;
            },
            ::core::option::Option::None => {},
        }

        let index = state.nodes.len();
        state.nodes.push(Node::new(kind));
        index
    }

    fn insert(&self, parent: usize, before: usize, child: NodeOrText<Handle>) {
        match {
            ::core::primitive::bool::then_some(parent == NONE || !self.charge_parse_work(1), ())
        } {
            ::core::option::Option::Some(()) => {
                return;
            },
            ::core::option::Option::None => {},
        }

        let child = match child {
            NodeOrText::AppendNode(child) => child.index,
            NodeOrText::AppendText(text) => {
                let previous = {
                    let state = self.state.borrow();

                    match {
                        ::core::primitive::bool::then_some(before == NONE, ())
                    } {
                        ::core::option::Option::Some(()) => {
                            state.nodes[parent].last
                        },
                        ::core::option::Option::None => {
                            state.nodes[before].prev
                        },
                    }
                };

                match {
                    ::core::primitive::bool::then_some(previous != NONE, ())
                } {
                    ::core::option::Option::Some(()) => {
                        let is_text = matches!(self.state.borrow().nodes[previous].kind, Kind::Text(_));

                        match ::core::primitive::bool::then_some(is_text, ()) {
                            ::core::option::Option::Some(()) => {
                                let count = self.state.borrow().nodes.len();

                                match {
                                    ::core::primitive::bool::then_some(!self.admit(count, text.len()), ())
                                } {
                                    ::core::option::Option::Some(()) => {
                                        return;
                                    },
                                    ::core::option::Option::None => {},
                                }

                                if let Kind::Text(existing) =
                                    &mut self.state.borrow_mut().nodes[previous].kind
                                {
                                    existing.push_tendril(&text);
                                }

                                return;
                            },
                            ::core::option::Option::None => {},
                        }
                    },
                    ::core::option::Option::None => {},
                }

                let bytes = text.len();
                self.allocate(Kind::Text(text), bytes)
            }
        };

        match {
            ::core::primitive::bool::then_some(child == NONE || self.failed(), ())
        } {
            ::core::option::Option::Some(()) => {
                return;
            },
            ::core::option::Option::None => {},
        }

        let mut state = self.state.borrow_mut();
        detach(&mut state.nodes, child);
        let previous = match {
            ::core::primitive::bool::then_some(before == NONE, ())
        } {
            ::core::option::Option::Some(()) => {
                state.nodes[parent].last
            },
            ::core::option::Option::None => {
                state.nodes[before].prev
            },
        };
        state.nodes[child].parent = parent;
        state.nodes[child].prev = previous;
        state.nodes[child].next = before;

        match {
            ::core::primitive::bool::then_some(previous == NONE, ())
        } {
            ::core::option::Option::Some(()) => {
                state.nodes[parent].first = child;
            },
            ::core::option::Option::None => {
                state.nodes[previous].next = child;
            },
        }

        match {
            ::core::primitive::bool::then_some(before == NONE, ())
        } {
            ::core::option::Option::Some(()) => {
                state.nodes[parent].last = child;
            },
            ::core::option::Option::None => {
                state.nodes[before].prev = child;
            },
        }
    }
}

fn detach(nodes: &mut [Node], index: usize) {
    let parent = nodes[index].parent;

    match {
        ::core::primitive::bool::then_some(parent == NONE, ())
    } {
        ::core::option::Option::Some(()) => {
            return;
        },
        ::core::option::Option::None => {},
    }

    let prev = nodes[index].prev;
    let next = nodes[index].next;

    match {
        ::core::primitive::bool::then_some(prev == NONE, ())
    } {
        ::core::option::Option::Some(()) => {
            nodes[parent].first = next;
        },
        ::core::option::Option::None => {
            nodes[prev].next = next;
        },
    }

    match {
        ::core::primitive::bool::then_some(next == NONE, ())
    } {
        ::core::option::Option::Some(()) => {
            nodes[parent].last = prev;
        },
        ::core::option::Option::None => {
            nodes[next].prev = prev;
        },
    }

    nodes[index].parent = NONE;
    nodes[index].prev = NONE;
    nodes[index].next = NONE;
}

#[must_use]
fn handle(index: usize, name: QualName) -> Handle {
    Handle {
        index,
        name,
        integration: false,
        attribute_bytes: 0,
    }
}

#[must_use]
fn container_name() -> QualName {
    QualName::new(None, ns!(html), LocalName::from("html"))
}

impl<S: SanitizeOutputSink> TreeSink for Builder<'_, S> {
    type Handle = Handle;
    type Output = Self;
    type ElemName<'a>
        = &'a Handle
    where
        Self: 'a;

    fn finish(self) -> Self {
        self
    }

    fn parse_error(&self, _: Cow<'static, str>) {}

    fn get_document(&self) -> Handle {
        handle(0, container_name())
    }

    fn elem_name<'a>(&'a self, node: &'a Handle) -> &'a Handle {
        let _charge_recorded_in_parser = self.charge_parse_work(1);
        node
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        let attribute_bytes = attrs.iter().fold(0usize, |bytes, attr| {
            bytes
                .saturating_add(attr.name.local.len())
                .saturating_add(attr.value.len())
        });
        let bytes = attrs.iter().fold(name.local.len(), |total, attr| {
            total
                .saturating_add(std::mem::size_of::<Attribute>())
                .saturating_add(attr.name.local.len())
                .saturating_add(attr.value.len())
        });
        let index = self.allocate(
            Kind::Element {
                name: name.clone(),
                attrs,
            },
            bytes,
        );

        match {
            ::core::primitive::bool::then_some(index != NONE && flags.template, ())
        } {
            ::core::option::Option::Some(()) => {
                let template = self.allocate(Kind::Container, 0);
                self.state.borrow_mut().nodes[index].template = template;
            },
            ::core::option::Option::None => {},
        }

        Handle {
            index,
            name,
            integration: flags.mathml_annotation_xml_integration_point,
            attribute_bytes,
        }
    }

    fn create_comment(&self, _: StrTendril) -> Handle {
        handle(self.allocate(Kind::Ignored, 0), container_name())
    }

    fn create_pi(&self, _: StrTendril, _: StrTendril) -> Handle {
        handle(self.allocate(Kind::Ignored, 0), container_name())
    }

    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        self.insert(parent.index, NONE, child);
    }

    fn append_based_on_parent_node(
        &self,
        element: &Handle,
        previous: &Handle,
        child: NodeOrText<Handle>,
    ) {
        match {
            ::core::primitive::bool::then_some(self.failed() || element.index == NONE, ())
        } {
            ::core::option::Option::Some(()) => {
                return;
            },
            ::core::option::Option::None => {},
        }

        let parent = self.state.borrow().nodes[element.index].parent;

        match {
            ::core::primitive::bool::then_some(parent == NONE, ())
        } {
            ::core::option::Option::Some(()) => {
                self.insert(previous.index, NONE, child);
            },
            ::core::option::Option::None => {
                self.insert(parent, element.index, child);
            },
        }
    }

    fn append_doctype_to_document(&self, _: StrTendril, _: StrTendril, _: StrTendril) {}

    fn get_template_contents(&self, node: &Handle) -> Handle {
        let index = match {
            ::core::primitive::bool::then_some(self.failed() || node.index == NONE, ())
        } {
            ::core::option::Option::Some(()) => {
                NONE
            },
            ::core::option::Option::None => {
                self.state.borrow().nodes[node.index].template
            },
        };
        handle(index, container_name())
    }

    fn same_node(&self, a: &Handle, b: &Handle) -> bool {
        let _charge_recorded_in_parser = self.charge_parse_work(1);
        a.index == b.index
    }

    fn set_quirks_mode(&self, _: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &Handle, node: NodeOrText<Handle>) {
        match {
            ::core::primitive::bool::then_some(self.failed() || sibling.index == NONE, ())
        } {
            ::core::option::Option::Some(()) => {
                return;
            },
            ::core::option::Option::None => {},
        }

        let parent = self.state.borrow().nodes[sibling.index].parent;
        self.insert(parent, sibling.index, node);
    }

    fn add_attrs_if_missing(&self, node: &Handle, attrs: Vec<Attribute>) {
        match {
            ::core::primitive::bool::then_some(self.failed() || node.index == NONE, ())
        } {
            ::core::option::Option::Some(()) => {
                return;
            },
            ::core::option::Option::None => {},
        }

        let bytes = attrs
            .iter()
            .map(|attr| {
                std::mem::size_of::<Attribute>()
                    + std::mem::size_of::<QualName>()
                    + attr.name.local.len()
                    + attr.value.len()
            })
            .sum();
        let count = self.state.borrow().nodes.len();

        match {
            ::core::primitive::bool::then_some(!self.admit(count, bytes), ())
        } {
            ::core::option::Option::Some(()) => {
                return;
            },
            ::core::option::Option::None => {},
        }

        let mut state = self.state.borrow_mut();
        let State {
            nodes,
            merged_attribute_names,
            ..
        } = &mut *state;

        if let Kind::Element {
            attrs: existing, ..
        } = &mut nodes[node.index].kind
        {
            let names = merged_attribute_names
                .entry(node.index)
                .or_insert_with(|| existing.iter().map(|attr| attr.name.clone()).collect());

            for attr in attrs {
                match {
                    ::core::primitive::bool::then_some(names.insert(attr.name.clone()), ())
                } {
                    ::core::option::Option::Some(()) => {
                        existing.push(attr);
                    },
                    ::core::option::Option::None => {},
                }
            }
        }
    }

    fn remove_from_parent(&self, node: &Handle) {
        match {
            ::core::primitive::bool::then_some(!self.failed() && node.index != NONE, ())
        } {
            ::core::option::Option::Some(()) => {
                detach(&mut self.state.borrow_mut().nodes, node.index);
            },
            ::core::option::Option::None => {},
        }
    }

    fn reparent_children(&self, node: &Handle, parent: &Handle) {
        match {
            ::core::primitive::bool::then_some(self.failed() || node.index == NONE || parent.index == NONE, ())
        } {
            ::core::option::Option::Some(()) => {
                return;
            },
            ::core::option::Option::None => {},
        }

        loop {
            let child = self.state.borrow().nodes[node.index].first;

            match {
                ::core::primitive::bool::then_some(child == NONE, ())
            } {
                ::core::option::Option::Some(()) => {
                    break;
                },
                ::core::option::Option::None => {},
            }

            self.insert(
                parent.index,
                NONE,
                NodeOrText::AppendNode(handle(child, container_name())),
            );

            match {
                ::core::primitive::bool::then_some(self.failed(), ())
            } {
                ::core::option::Option::Some(()) => {
                    break;
                },
                ::core::option::Option::None => {},
            }
        }
    }

    fn is_mathml_annotation_xml_integration_point(&self, node: &Handle) -> bool {
        let _charge_recorded_in_parser = self.charge_parse_work(1);
        node.integration
    }
}

#[derive(Clone, Copy)]
pub(super) enum Output {
    Html,
    Xhtml,
}

pub(super) fn sanitize<S: SanitizeOutputSink>(
    text: &str,
    cfg: &DompurifyEffectiveConfig,
    sink: &S,
    output: Output,
) -> Result<String, SanitizeFailure<S::Error>> {
    parse_and_serialize(text, Some(cfg), sink, output, ns!(html))
}

pub(super) fn xhtml_from_sanitized_svg_fragment<S: SanitizeOutputSink>(
    text: &str,
    sink: &S,
) -> Result<String, SanitizeFailure<S::Error>> {
    // Architecture assigns already sanitized HTML to an SVG div's innerHTML. The
    // browser then constructs a foreign-content fragment without sanitizing it again.
    parse_and_serialize(text, None, sink, Output::Xhtml, ns!(svg))
}

pub(super) fn parse_and_serialize<S: SanitizeOutputSink>(
    text: &str,
    cfg: Option<&DompurifyEffectiveConfig>,
    sink: &S,
    output: Output,
    context: Namespace,
) -> Result<String, SanitizeFailure<S::Error>> {
    sink.checkpoint().map_err(SanitizeFailure::Output)?;
    sink.check_parse_storage(text.len(), 0, 0)
        .map_err(SanitizeFailure::Output)?;
    let builder = Builder {
        state: RefCell::new(State {
            nodes: vec![Node::new(Kind::Container)],
            retained: std::mem::size_of::<Node>(),
            error: None,
            merged_attribute_names: HashMap::new(),
            parse_work: 0,
            total_parse_work: 0,
            parse_work_limit: sink.parse_work_limit(text.len()),
        }),
        sink,
        input_bytes: text.len(),
    };
    let mut options = html5ever::ParseOpts::default();
    let tree_builder = match cfg {
        ::core::option::Option::Some(_) => {
            // DOMPurify's default DOMParser uses an inert HTML document, then sanitizes
            // and returns its body. Head/template placement is not body-fragment parsing.
            options.tree_builder.scripting_enabled = false;
            TreeBuilder::new(builder, options.tree_builder)
        },
        ::core::option::Option::None => {
            let local_name = match {
                ::core::primitive::bool::then_some(context == ns!(html), ())
            } {
                ::core::option::Option::Some(()) => { "div" },
                ::core::option::Option::None => { "svg" },
            };
            let context_name = QualName::new(None, context.clone(), LocalName::from(local_name));
            let context_element =
                html5ever::tree_builder::create_element(&builder, context_name, Vec::new());
            let tree_builder =
                TreeBuilder::new_for_fragment(builder, context_element, None, options.tree_builder);
            options.tokenizer.initial_state =
                Some(tree_builder.tokenizer_state_for_context_elem(false));
            tree_builder
        },
    };
    let parser = Tokenizer::new(
        StoppingTreeBuilder {
            inner: tree_builder,
            stopped: std::cell::Cell::new(false),
            complete_token: std::cell::Cell::new(false),
        },
        options.tokenizer,
    );
    let input_queue = BufferQueue::default();
    let mut cursor = 0;
    let mut pending_token_bytes: usize = 0;

    while cursor < text.len() {
        sink.checkpoint().map_err(SanitizeFailure::Output)?;
        let mut end = (cursor + CHUNK_BYTES).min(text.len());

        while !text.is_char_boundary(end) {
            end -= 1;
        }

        // The tokenizer checks each new attribute against the attributes already
        // present on its unfinished tag, before it emits any token or TreeSink call.
        // Prepay a conservative quadratic byte-work envelope for the current token;
        // completed tokens reset it, so ordinary sequences of short tags stay linear.
        let next_pending = pending_token_bytes.saturating_add(end - cursor);
        let token_work = next_pending
            .saturating_mul(next_pending)
            .div_ceil(WORK_BYTES_PER_UNIT)
            .saturating_sub(
                pending_token_bytes
                    .saturating_mul(pending_token_bytes)
                    .div_ceil(WORK_BYTES_PER_UNIT),
            );
        let _token_charge_recorded = parser.sink.inner.sink.charge_parse_work(token_work);
        let _token_flush_recorded = parser.sink.inner.sink.flush_parse_work();

        match parser.sink.inner.sink.state.borrow_mut().error.take() {
            Some(error) => {
                return Err(error);
            },
            ::core::option::Option::None => {},
        }

        parser.sink.complete_token.set(false);
        input_queue.push_back(StrTendril::from_slice(&text[cursor..end]));

        while !matches!(parser.feed(&input_queue), TokenizerResult::Done) {}

        pending_token_bytes = match {
            ::core::primitive::bool::then_some(parser.sink.complete_token.get(), ())
        } {
            ::core::option::Option::Some(()) => {
                // A complete token may be followed by an unfinished tag in this same
                // chunk. Retaining one entire chunk bounds that unknown trailing prefix.
                CHUNK_BYTES
            },
            ::core::option::Option::None => {
                next_pending
            },
        };
        let _chunk_flush_recorded = parser.sink.inner.sink.flush_parse_work();

        // TreeSink cannot return errors. A rejected callback preserves real element
        // names/identity until the current HTML5 token completes; subsequent tokens
        // are not forwarded to the tree builder. The tokenizer only drains this
        // bounded chunk before the parser is discarded without EOF or unwinding.
        match parser.sink.inner.sink.state.borrow_mut().error.take() {
            Some(error) => {
                return Err(error);
            },
            ::core::option::Option::None => {},
        }

        cursor = end;
    }

    parser.end();
    let builder = parser.sink.inner.sink.finish();
    let _final_flush_recorded = builder.flush_parse_work();
    let root = match cfg {
        ::core::option::Option::Some(_) => {
            builder.state.borrow().nodes.iter().position(|node| {
                matches!(&node.kind, Kind::Element { name, .. } if name.ns == ns!(html) && &*name.local == "body")
            }).unwrap_or(NONE)
        },
        ::core::option::Option::None => {
            builder.state.borrow().nodes[0].first
        },
    };

    match {
        ::core::primitive::bool::then_some(cfg.is_some() && root != NONE && !builder.failed(), ())
    } {
        ::core::option::Option::Some(()) => {
            // DOMPurify restores precisely this prefix after DOMParser moves leading
            // whitespace out of the body (pinned _initDocument implementation).
            let prefix = text
                .bytes()
                .take_while(|byte| matches!(byte, b'\r' | b'\n' | b'\t' | b' '))
                .count();

            match prefix {
                0 => {},
                _ => {
                    let first = builder.state.borrow().nodes[root].first;
                    builder.insert(
                        root,
                        first,
                        NodeOrText::AppendText(StrTendril::from_slice(&text[..prefix])),
                    );
                },
            }
        },
        ::core::option::Option::None => {},
    }

    let mut state = builder.state.into_inner();

    match state.error {
        Some(error) => {
            return Err(error);
        },
        ::core::option::Option::None => {},
    }

    match {
        ::core::primitive::bool::then_some(root == NONE, ())
    } {
        ::core::option::Option::Some(()) => {
            return Ok(String::new());
        },
        ::core::option::Option::None => {},
    }

    serialize(&mut state.nodes, root, cfg, sink, output)
}

#[must_use]
fn qualified_name(name: &QualName) -> Cow<'_, str> {
    match &name.prefix {
        Some(prefix) => Cow::Owned(format!("{prefix}:{}", name.local)),
        None => Cow::Borrowed(&name.local),
    }
}

#[must_use]
fn is_void(name: &QualName) -> bool {
    name.ns == ns!(html)
        && matches!(
            name.local.as_ref(),
            "area"
                | "base"
                | "br"
                | "col"
                | "embed"
                | "hr"
                | "img"
                | "input"
                | "link"
                | "meta"
                | "param"
                | "source"
                | "track"
                | "wbr"
        )
}

#[must_use]
fn is_xml_local_name(name: &str) -> bool {
    // XML 1.0 (Fifth Edition) NameStartChar/NameChar, excluding namespace colons.
    #[must_use]
    fn start(ch: char) -> bool {
        matches!(ch, 'A'..='Z' | '_' | 'a'..='z' | '\u{c0}'..='\u{d6}' | '\u{d8}'..='\u{f6}'
            | '\u{f8}'..='\u{2ff}' | '\u{370}'..='\u{37d}' | '\u{37f}'..='\u{1fff}'
            | '\u{200c}'..='\u{200d}' | '\u{2070}'..='\u{218f}' | '\u{2c00}'..='\u{2fef}'
            | '\u{3001}'..='\u{d7ff}' | '\u{f900}'..='\u{fdcf}' | '\u{fdf0}'..='\u{fffd}' | '\u{10000}'..='\u{effff}')
    }

    let mut chars = name.chars();
    chars.next().is_some_and(start) && chars.all(|ch| start(ch) || matches!(ch, '-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}'))
}

#[must_use]
fn valid_namespace(name: &QualName, parent: Option<&QualName>, context: &Namespace) -> bool {
    use crate::generated::dompurify_defaults as defaults;
    let tag = name.local.to_ascii_lowercase();
    let tag = &*tag;
    let parent_ns = parent.map_or(context, |parent| &parent.ns);
    let parent_tag = parent.map_or("template", |parent| &*parent.local);

    match {
        ::core::primitive::bool::then_some(name.ns == ns!(svg), ())
    } {
        ::core::option::Option::Some(()) => {
            match {
                ::core::primitive::bool::then_some(*parent_ns == ns!(html), ())
            } {
                ::core::option::Option::Some(()) => {
                    return tag == "svg";
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(*parent_ns == ns!(mathml), ())
            } {
                ::core::option::Option::Some(()) => {
                    return tag == "svg"
                        && (parent_tag == "annotation-xml"
                            || defaults::DEFAULT_MATHML_TEXT_INTEGRATION_POINTS.contains(&parent_tag));
                },
                ::core::option::Option::None => {},
            }

            return defaults::ALL_SVG_TAGS.contains(&tag);
        },
        ::core::option::Option::None => {},
    }

    match {
        ::core::primitive::bool::then_some(name.ns == ns!(mathml), ())
    } {
        ::core::option::Option::Some(()) => {
            match {
                ::core::primitive::bool::then_some(*parent_ns == ns!(html), ())
            } {
                ::core::option::Option::Some(()) => {
                    return tag == "math";
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(*parent_ns == ns!(svg), ())
            } {
                ::core::option::Option::Some(()) => {
                    return tag == "math"
                        && defaults::DEFAULT_HTML_INTEGRATION_POINTS.contains(&parent_tag);
                },
                ::core::option::Option::None => {},
            }

            return defaults::ALL_MATHML_TAGS.contains(&tag);
        },
        ::core::option::Option::None => {},
    }

    match {
        ::core::primitive::bool::then_some(name.ns == ns!(html), ())
    } {
        ::core::option::Option::Some(()) => {
            match {
                ::core::primitive::bool::then_some(*parent_ns == ns!(svg)
                && !defaults::DEFAULT_HTML_INTEGRATION_POINTS.contains(&parent_tag), ())
            } {
                ::core::option::Option::Some(()) => {
                    return false;
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(*parent_ns == ns!(mathml)
                && !defaults::DEFAULT_MATHML_TEXT_INTEGRATION_POINTS.contains(&parent_tag), ())
            } {
                ::core::option::Option::Some(()) => {
                    return false;
                },
                ::core::option::Option::None => {},
            }

            return !defaults::ALL_MATHML_TAGS.contains(&tag)
                && (defaults::COMMON_SVG_AND_HTML_ELEMENTS.contains(&tag)
                    || !defaults::ALL_SVG_TAGS.contains(&tag));
        },
        ::core::option::Option::None => {},
    }

    false
}

#[must_use]
fn is_literal_html_tag(tag: &str) -> bool {
    matches!(
        tag,
        "style" | "script" | "xmp" | "iframe" | "noembed" | "noframes" | "plaintext" | "noscript"
    )
}

fn unsafe_literal_html<S: SanitizeOutputSink>(
    nodes: &[Node],
    index: usize,
    sink: &S,
) -> Result<bool, SanitizeFailure<S::Error>> {
    // DOMPurify SAFE_FOR_XML rejects element children or markup-like literal text,
    // including noscript. Match across text nodes without concatenation or recursion.
    let mut child = nodes[index].first;
    let mut markup_start = false;

    while child != NONE {
        sink.checkpoint().map_err(SanitizeFailure::Output)?;

        match &nodes[child].kind {
            Kind::Element { .. } => return Ok(true),
            Kind::Text(text) => {
                for (offset, byte) in text.bytes().enumerate() {
                    match {
                        ::core::primitive::bool::then_some(offset % CHUNK_BYTES == 0, ())
                    } {
                        ::core::option::Option::Some(()) => {
                            sink.checkpoint().map_err(SanitizeFailure::Output)?;
                        },
                        ::core::option::Option::None => {},
                    }

                    match {
                        ::core::primitive::bool::then_some(markup_start
                        && (byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'/' | b'!')), ())
                    } {
                        ::core::option::Option::Some(()) => {
                            return Ok(true);
                        },
                        ::core::option::Option::None => {},
                    }

                    markup_start = byte == b'<';
                }
            }
            _ => {}
        }

        child = nodes[child].next;
    }

    Ok(false)
}

struct Writer<'a, S> {
    value: Vec<u8>,
    sink: &'a S,
}

impl<S: SanitizeOutputSink> Writer<'_, S> {
    fn push(&mut self, value: &str) -> Result<(), SanitizeFailure<S::Error>> {
        self.sink
            .push_output_chunk(&mut self.value, value.as_bytes())
            .map_err(SanitizeFailure::Output)
    }

    fn name(&mut self, name: &QualName) -> Result<(), SanitizeFailure<S::Error>> {
        match &name.prefix {
            Some(prefix) => {
                self.push(prefix)?;
                self.push(":")?;
            },
            ::core::option::Option::None => {},
        }

        self.push(&name.local)
    }

    fn escaped(
        &mut self,
        text: &str,
        attr: bool,
        escape_gt: bool,
        xhtml: bool,
    ) -> Result<(), SanitizeFailure<S::Error>> {
        let mut start = 0;

        for (i, (offset, ch)) in text.char_indices().enumerate() {
            match {
                ::core::primitive::bool::then_some(i % 64 == 0, ())
            } {
                ::core::option::Option::Some(()) => {
                    self.sink.checkpoint().map_err(SanitizeFailure::Output)?;
                },
                ::core::option::Option::None => {},
            }

            let replacement = match ch {
                '&' => Some("&amp;"),
                '<' => Some("&lt;"),
                '>' if escape_gt || xhtml => Some("&gt;"),
                '"' if attr => Some("&quot;"),
                '\n' if attr && xhtml => Some("&#10;"),
                '\r' if attr && xhtml => Some("&#13;"),
                '\t' if attr && xhtml => Some("&#9;"),
                '\u{a0}' if !xhtml => Some("&nbsp;"),
                '\u{0}'..='\u{8}'
                | '\u{b}'
                | '\u{c}'
                | '\u{e}'..='\u{1f}'
                | '\u{fffe}'
                | '\u{ffff}'
                    if xhtml =>
                {
                    Some("")
                }
                _ => None,
            };

            match replacement {
                Some(replacement) => {
                    self.push(&text[start..offset])?;
                    self.push(replacement)?;
                    start = offset + ch.len_utf8();
                },
                ::core::option::Option::None => {},
            }
        }

        self.push(&text[start..])
    }
}

fn serialize<S: SanitizeOutputSink>(
    nodes: &mut [Node],
    root: usize,
    cfg: Option<&DompurifyEffectiveConfig>,
    sink: &S,
    mode: Output,
) -> Result<String, SanitizeFailure<S::Error>> {
    let xhtml = matches!(mode, Output::Xhtml);
    // All XHTML fragments are embedded beneath an XHTML parent, independently of
    // the HTML5 parsing context that produced their expanded element names.
    let context = ns!(html);
    let mut out = Writer {
        value: sink.output_buffer(0).map_err(SanitizeFailure::Output)?,
        sink,
    };
    // The HTML5 fragment builder's synthetic html root is not part of the fragment.
    let mut stack = Vec::new();
    let mut child = nodes[root].last;

    while child != NONE {
        stack.push((child, false, None));
        child = nodes[child].prev;
    }

    while let Some((index, close, parent_name)) = stack.pop() {
        sink.checkpoint().map_err(SanitizeFailure::Output)?;

        if !close
            && cfg.is_some_and(|cfg| cfg.safe_for_xml)
            && let Kind::Element { name, .. } = &nodes[index].kind
            && name.ns == ns!(html)
            && is_literal_html_tag(&name.local)
            && unsafe_literal_html(nodes, index, sink)?
        {
            continue;
        }

        let parent_ns = parent_name
            .as_ref()
            .map_or(&context, |name: &QualName| &name.ns);
        let mut child_parent = parent_name.clone();
        let mut descend = true;
        let raw_html_text = !xhtml
            && parent_name.as_ref().is_some_and(|name| {
                name.ns == ns!(html)
                    && matches!(
                        &*name.local,
                        "style"
                            | "script"
                            | "xmp"
                            | "iframe"
                            | "noembed"
                            | "noframes"
                            | "plaintext"
                    )
            });

        match &mut nodes[index].kind {
            Kind::Text(text) if raw_html_text => out.push(text)?,
            Kind::Text(text) => out.escaped(text, false, true, xhtml)?,
            Kind::Element { name, attrs } => {
                let local = name.local.to_ascii_lowercase();

                match ::core::primitive::bool::then_some(close, ()) {
                    ::core::option::Option::Some(()) => {
                        out.push("</")?;
                        out.name(name)?;
                        out.push(">")?;
                        continue;
                    },
                    ::core::option::Option::None => {},
                }

                let allowed = cfg.is_none_or(|cfg| {
                    cfg.allowed_tags.contains(local.as_str())
                        && !cfg.forbid_tags.contains(local.as_str())
                });

                match {
                    ::core::primitive::bool::then_some(cfg.is_some_and(|cfg| {
                        matches!(local.as_str(), "script" | "iframe" | "style")
                            || (!allowed
                                && (!cfg.keep_content || cfg.forbid_contents.contains(local.as_str())))
                    }), ())
                } {
                    ::core::option::Option::Some(()) => {
                        descend = false;
                    },
                    ::core::option::Option::None => match ::core::primitive::bool::then_some(allowed, ()) {
                        ::core::option::Option::Some(()) => {
                            match {
                                ::core::primitive::bool::then_some(cfg.is_some() && !valid_namespace(name, parent_name.as_ref(), &context), ())
                            } {
                                ::core::option::Option::Some(()) => {
                                    continue;
                                },
                                ::core::option::Option::None => {},
                            }

                            match {
                                ::core::primitive::bool::then_some(xhtml && !is_xml_local_name(&name.local), ())
                            } {
                                ::core::option::Option::Some(()) => {
                                    return Err(SanitizeFailure::RejectedInput);
                                },
                                ::core::option::Option::None => {},
                            }

                            match ::core::primitive::bool::then_some(xhtml, ()) {
                                ::core::option::Option::Some(()) => {
                                    attrs.retain(|attr| {
                                        is_xml_local_name(&attr.name.local)
                                            && attr.name.prefix.as_ref().is_none_or(|prefix| {
                                                matches!(&**prefix, "xml" | "xlink" | "xmlns")
                                            })
                                    });
                                },
                                ::core::option::Option::None => {},
                            }

                            match cfg {
                                Some(cfg) => {
                                    let target = (&*local == "a")
                                        .then(|| {
                                            attrs
                                                .iter()
                                                .find(|a| &*a.name.local == "target")
                                                .map(|a| a.value.clone())
                                        })
                                        .flatten();

                                    match target {
                                        Some(target) => {
                                            if let Some(existing) = attrs
                                                .iter_mut()
                                                .find(|attr| &*attr.name.local == "data-temp-href-target")
                                            {
                                                existing.value = target;
                                            } else {
                                                attrs.push(Attribute {
                                                    name: QualName::new(
                                                        None,
                                                        ns!(),
                                                        LocalName::from("data-temp-href-target"),
                                                    ),
                                                    value: target,
                                                });
                                            }
                                        },
                                        ::core::option::Option::None => {},
                                    }

                                    attrs.retain_mut(|attr| {
                                        let key = qualified_name(&attr.name);
                                        let key = match {
                                            ::core::primitive::bool::then_some(key.bytes().any(|byte| byte.is_ascii_uppercase()), ())
                                        } {
                                            ::core::option::Option::Some(()) => {
                                                Cow::Owned(key.to_ascii_lowercase())
                                            },
                                            ::core::option::Option::None => {
                                                key
                                            },
                                        };
                                        let value =
                                            super::dompurify_normalize_dom_attribute_value(&key, &attr.value);

                                        match {
                                            ::core::primitive::bool::then_some(!super::dompurify_is_valid_attribute(cfg, &local, &key, value), ())
                                        } {
                                            ::core::option::Option::Some(()) => {
                                                return false;
                                            },
                                            ::core::option::Option::None => {},
                                        }

                                        match {
                                            ::core::primitive::bool::then_some(value != attr.value.as_ref(), ())
                                        } {
                                            ::core::option::Option::Some(()) => {
                                                attr.value = StrTendril::from_slice(value);
                                            },
                                            ::core::option::Option::None => {},
                                        }

                                        true
                                    });

                                    if &*local == "a"
                                        && let Some(i) = attrs
                                            .iter()
                                            .position(|a| &*a.name.local == "data-temp-href-target")
                                    {
                                        let value = attrs.remove(i).value;
                                        attrs.retain(|a| {
                                            &*a.name.local != "target"
                                                && (value.as_ref() != "_blank" || &*a.name.local != "rel")
                                        });
                                        attrs.push(Attribute {
                                            name: QualName::new(None, ns!(), LocalName::from("target")),
                                            value: value.clone(),
                                        });

                                        match {
                                            ::core::primitive::bool::then_some(value.as_ref() == "_blank", ())
                                        } {
                                            ::core::option::Option::Some(()) => {
                                                attrs.push(Attribute {
                                                    name: QualName::new(None, ns!(), LocalName::from("rel")),
                                                    value: StrTendril::from_slice("noopener"),
                                                });
                                            },
                                            ::core::option::Option::None => {},
                                        }
                                    }
                                },
                                ::core::option::Option::None => {},
                            }

                            out.push("<")?;
                            out.name(name)?;

                            match {
                                ::core::primitive::bool::then_some(xhtml
                                && (name.ns != *parent_ns
                                    || attrs.iter().any(|attr| &*attr.name.local == "xmlns")), ())
                            } {
                                ::core::option::Option::Some(()) => {
                                    out.push(" xmlns=\"")?;
                                    out.escaped(name.ns.as_ref(), true, false, true)?;
                                    out.push("\"")?;
                                },
                                ::core::option::Option::None => {},
                            }

                            let mut wrote_xlink = false;

                            for attr in attrs.iter() {
                                match {
                                    ::core::primitive::bool::then_some(xhtml && (&*attr.name.local == "xmlns" || attr.name.ns == ns!(xmlns)), ())
                                } {
                                    ::core::option::Option::Some(()) => {
                                        continue;
                                    },
                                    ::core::option::Option::None => {},
                                }

                                match {
                                    ::core::primitive::bool::then_some(xhtml && attr.name.ns == ns!(xlink) && !wrote_xlink, ())
                                } {
                                    ::core::option::Option::Some(()) => {
                                        out.push(" xmlns:xlink=\"http://www.w3.org/1999/xlink\"")?;
                                        wrote_xlink = true;
                                    },
                                    ::core::option::Option::None => {},
                                }

                                out.push(" ")?;
                                out.name(&attr.name)?;
                                out.push("=\"")?;
                                out.escaped(&attr.value, true, false, xhtml)?;
                                out.push("\"")?;
                            }

                            match {
                                ::core::primitive::bool::then_some(is_void(name), ())
                            } {
                                ::core::option::Option::Some(()) => {
                                    out.push(match ::core::primitive::bool::then_some(xhtml, ()) {
                                        ::core::option::Option::Some(()) => { " />" },
                                        ::core::option::Option::None => { ">" },
                                    })?;
                                },
                                ::core::option::Option::None => {
                                    out.push(">")?;
                                    stack.push((index, true, parent_name.clone()));
                                },
                            }

                            child_parent = Some(name.clone());
                        },
                        ::core::option::Option::None => {},
                    },
                }
            }
            Kind::Ignored => descend = false,
            Kind::Container => {}
        }

        match ::core::primitive::bool::then_some(descend, ()) {
            ::core::option::Option::Some(()) => {
                let contents = nodes[index].template;
                let mut child = match {
                    ::core::primitive::bool::then_some(contents == NONE, ())
                } {
                    ::core::option::Option::Some(()) => {
                        nodes[index].last
                    },
                    ::core::option::Option::None => {
                        nodes[contents].last
                    },
                };

                while child != NONE {
                    stack.push((child, false, child_parent.clone()));
                    child = nodes[child].prev;
                }
            },
            ::core::option::Option::None => {},
        }
    }

    String::from_utf8(out.value).map_err(|_| SanitizeFailure::InvalidUtf8Output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_tree_does_not_process_subsequent_tokens() {
        let output_sink = super::super::StringOutputSink;
        let builder = Builder {
            state: RefCell::new(State {
                nodes: vec![Node::new(Kind::Container)],
                retained: std::mem::size_of::<Node>(),
                error: None,
                merged_attribute_names: HashMap::new(),
                parse_work: 0,
                total_parse_work: 0,
                parse_work_limit: None,
            }),
            sink: &output_sink,
            input_bytes: 64,
        };
        let stopping = StoppingTreeBuilder {
            inner: TreeBuilder::new(builder, Default::default()),
            stopped: std::cell::Cell::new(false),
            complete_token: std::cell::Cell::new(false),
        };
        let token = || {
            html5ever::tokenizer::Token::TagToken(html5ever::tokenizer::Tag {
                kind: html5ever::tokenizer::StartTag,
                name: LocalName::from("div"),
                self_closing: false,
                attrs: Vec::new(),
                had_duplicate_attributes: false,
            })
        };
        let _ = stopping.process_token(token(), 1);
        let before = {
            let state = stopping.inner.sink.state.borrow();
            (state.nodes.len(), state.total_parse_work)
        };
        stopping.inner.sink.state.borrow_mut().error = Some(SanitizeFailure::RejectedInput);
        for _ in 0..8 {
            assert!(matches!(
                stopping.process_token(token(), 1),
                TokenSinkResult::Continue
            ));
        }
        stopping.end();
        let state = stopping.inner.sink.state.borrow();
        assert!(stopping.stopped.get());
        assert_eq!((state.nodes.len(), state.total_parse_work), before);
        assert!(matches!(state.error, Some(SanitizeFailure::RejectedInput)));
    }
}
