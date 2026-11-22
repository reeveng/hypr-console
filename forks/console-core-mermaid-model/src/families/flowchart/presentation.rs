//! Source-faithful labels and frozen Flowchart render facts.

use super::{FlowEdge, FlowNode, FlowSubgraph, FlowchartModel, FlowchartSubgraphStyles};
use crate::{OperationControl, OperationControlResult};
use console_core_perfect_hash::collections::{FixedMap, FixedSet};

/// Source-faithful labels indexed by canonical model identity and encounter ordinal.
#[derive(Debug, Clone, Default)]
pub struct FlowchartRenderLabelSources {
    nodes: FixedMap<String, String>,
    edges: Vec<(usize, String)>,
    subgraphs: Vec<Option<FlowSubgraphRenderLabelSource>>,
}

#[derive(Debug, Clone)]
struct FlowSubgraphRenderLabelSource {
    id: String,
    source: String,
}

impl FlowchartRenderLabelSources {
    #[must_use]
    pub fn node_label_for_render<'a>(&'a self, node: &'a FlowNode) -> Option<&'a str> {
        self.nodes
            .get(&node.id)
            .map(String::as_str)
            .or(node.label.as_deref())
    }

    #[must_use]
    pub fn edge_label_for_render<'a>(
        &'a self,
        semantic_index: usize,
        edge: &'a FlowEdge,
    ) -> Option<&'a str> {
        match self.edges
            .binary_search_by_key(&semantic_index, |(index, _)| *index) { ::core::result::Result::Ok(value) => ::core::option::Option::Some(value), ::core::result::Result::Err(_insertion_position) => ::core::option::Option::None }
            .map(|position| self.edges[position].1.as_str())
            .or(edge.label.as_deref())
    }

    #[must_use]
    pub fn subgraph_title_for_render<'a>(
        &'a self,
        declaration_ordinal: usize,
        subgraph: &'a FlowSubgraph,
    ) -> &'a str {
        self.subgraphs
            .get(declaration_ordinal)
            .and_then(Option::as_ref)
            .filter(|source| source.id == subgraph.id)
            .map(|source| source.source.as_str())
            .unwrap_or(subgraph.title.as_str())
    }

    pub fn insert_node(&mut self, id: String, source: String) {
        self.nodes.insert(id, source);
    }

    pub fn insert_edge(&mut self, semantic_index: usize, source: String) {
        match {
            ::core::primitive::bool::then_some(self
            .edges
            .last()
            .is_none_or(|(last_index, _)| *last_index < semantic_index), ())
        } {
            ::core::option::Option::Some(()) => {
                self.edges.push((semantic_index, source));
                return;
            },
            ::core::option::Option::None => {},
        }

        match self
            .edges
            .binary_search_by_key(&semantic_index, |(index, _)| *index)
        {
            Ok(position) => self.edges[position].1 = source,
            Err(position) => self.edges.insert(position, (semantic_index, source)),
        }
    }

    pub fn insert_subgraph(
        &mut self,
        id: String,
        declaration_ordinal: usize,
        source: Option<String>,
    ) {
        match source {
            Some(source) => {
                match {
                    ::core::primitive::bool::then_some(self.subgraphs.len() <= declaration_ordinal, ())
                } {
                    ::core::option::Option::Some(()) => {
                        self.subgraphs.resize_with(declaration_ordinal + 1, || None);
                    },
                    ::core::option::Option::None => {},
                }

                self.subgraphs[declaration_ordinal] =
                    Some(FlowSubgraphRenderLabelSource { id, source });
            },
            ::core::option::Option::None => {},
        }
    }

    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        self.nodes
            .iter()
            .fold(0usize, |total, (id, label)| {
                total.saturating_add(id.len()).saturating_add(label.len())
            })
            .saturating_add(
                self.edges
                    .iter()
                    .map(|(_, source)| source.len())
                    .sum::<usize>(),
            )
            .saturating_add(
                self.subgraphs
                    .iter()
                    .flatten()
                    .fold(0usize, |total, source| {
                        total
                            .saturating_add(source.id.len())
                            .saturating_add(source.source.len())
                    }),
            )
    }
}

/// Frozen render facts and a shared view of canonical authored subgraph CSS.
#[doc(hidden)]
#[derive(Debug, Clone, Default)]
pub struct FlowchartRenderContext {
    labels: FlowchartRenderLabelSources,
    styles: FlowchartSubgraphStyles,
    collapsed_subgraphs: FixedSet<String>,
    collapsed_replacements: FixedMap<String, String>,
    subgraph_color_ordinals: FixedMap<String, usize>,
    node_dom_indices: FixedMap<String, usize>,
}

impl FlowchartRenderContext {
    /// Inherits canonical CSS without adding labels, collapse facts, or source-assigned IDs.
    /// The canonical model already accounts for the shared CSS allocation.
    #[must_use]
    pub fn from_model(model: &FlowchartModel) -> Self {
        Self {
            styles: model.subgraph_vertex_styles.clone(),
            ..Self::default()
        }
    }

    /// Checks the relationships represented by these completed output facts.
    ///
    /// The caller admits model complexity and retained context before allocating these linear
    /// indexes. Registration history and hidden-container ancestry cannot be recovered from the
    /// projected graph; this method does not authenticate that history or impose a DOM upper bound.
    pub fn validate_for_model_controlled(
        &self,
        model: &FlowchartModel,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(), &'static str>> {
        control.checkpoint()?;
        let mut node_ids = FixedSet::default();

        for node in &model.nodes {
            control.checkpoint()?;
            node_ids.insert(node.id.as_str());
        }

        let mut subgraph_ids = FixedSet::default();

        for subgraph in &model.subgraphs {
            control.checkpoint()?;
            subgraph_ids.insert(subgraph.id.as_str());
        }

        for id in self.labels.nodes.keys() {
            control.checkpoint()?;

            match {
                ::core::primitive::bool::then_some(!node_ids.contains(id.as_str()), ())
            } {
                ::core::option::Option::Some(()) => {
                    return Ok(Err("flowchart node label refers to an unknown node"));
                },
                ::core::option::Option::None => {},
            }
        }

        let mut previous_edge_index = None;

        for (index, _) in &self.labels.edges {
            control.checkpoint()?;

            match {
                ::core::primitive::bool::then_some(*index >= model.edges.len(), ())
            } {
                ::core::option::Option::Some(()) => {
                    return Ok(Err("flowchart edge label semantic index is out of range"));
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(previous_edge_index.is_some_and(|previous| previous >= *index), ())
            } {
                ::core::option::Option::Some(()) => {
                    return Ok(Err(
                        "flowchart edge label semantic indices are not strictly increasing",
                    ));
                },
                ::core::option::Option::None => {},
            }

            previous_edge_index = Some(*index);
        }

        control.checkpoint()?;

        match {
            ::core::primitive::bool::then_some(self.labels.subgraphs.len() > model.subgraphs.len(), ())
        } {
            ::core::option::Option::Some(()) => {
                return Ok(Err(
                    "flowchart subgraph label declaration ordinal is out of range",
                ));
            },
            ::core::option::Option::None => {},
        }

        for (ordinal, source) in self.labels.subgraphs.iter().enumerate() {
            control.checkpoint()?;

            if let Some(source) = source
                && model.subgraphs[ordinal].id != source.id
            {
                return Ok(Err(
                    "flowchart subgraph label ID does not match its declaration ordinal",
                ));
            }
        }

        match model
            .subgraph_vertex_styles
            .validate_for_model_controlled(model, control)? {
            Err(message) => {
                return Ok(Err(message));
            },
            ::core::result::Result::Ok(_) => {},
        }

        match self.styles.validate_for_model_controlled(model, control)? {
            Err(message) => {
                return Ok(Err(message));
            },
            ::core::result::Result::Ok(_) => {},
        }

        for id in &self.collapsed_subgraphs {
            control.checkpoint()?;

            match {
                ::core::primitive::bool::then_some(!subgraph_ids.contains(id.as_str()), ())
            } {
                ::core::option::Option::Some(()) => {
                    return Ok(Err(
                        "flowchart collapsed subgraph refers to an unknown subgraph",
                    ));
                },
                ::core::option::Option::None => {},
            }
        }

        for (id, replacement) in &self.collapsed_replacements {
            control.checkpoint()?;

            // All canonical nodes remain in the identity domain even when collapse hides them.
            match {
                ::core::primitive::bool::then_some(!node_ids.contains(id.as_str()) && !subgraph_ids.contains(id.as_str()), ())
            } {
                ::core::option::Option::Some(()) => {
                    return Ok(Err(
                        "flowchart collapsed replacement refers to an unknown node or subgraph",
                    ));
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(!self.collapsed_subgraphs.contains(replacement), ())
            } {
                ::core::option::Option::Some(()) => {
                    return Ok(Err(
                        "flowchart collapsed replacement target is not a collapsed subgraph",
                    ));
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(id == replacement, ())
            } {
                ::core::option::Option::Some(()) => {
                    return Ok(Err("flowchart collapsed replacement targets itself"));
                },
                ::core::option::Option::None => {},
            }
        }

        let mut palette_ordinals = FixedSet::default();

        for (id, ordinal) in &self.subgraph_color_ordinals {
            control.checkpoint()?;

            match {
                ::core::primitive::bool::then_some(!subgraph_ids.contains(id.as_str()), ())
            } {
                ::core::option::Option::Some(()) => {
                    return Ok(Err(
                        "flowchart palette ordinal refers to an unknown subgraph",
                    ));
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(*ordinal >= subgraph_ids.len(), ())
            } {
                ::core::option::Option::Some(()) => {
                    return Ok(Err("flowchart palette ordinal is out of range"));
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(!palette_ordinals.insert(*ordinal), ())
            } {
                ::core::option::Option::Some(()) => {
                    return Ok(Err("flowchart palette ordinals must be distinct"));
                },
                ::core::option::Option::None => {},
            }
        }

        let mut dom_ordinals = FixedSet::default();

        for (id, ordinal) in &self.node_dom_indices {
            control.checkpoint()?;

            match {
                ::core::primitive::bool::then_some(!node_ids.contains(id.as_str()) && !subgraph_ids.contains(id.as_str()), ())
            } {
                ::core::option::Option::Some(()) => {
                    return Ok(Err(
                        "flowchart DOM ordinal refers to an unknown node or subgraph",
                    ));
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(!dom_ordinals.insert(*ordinal), ())
            } {
                ::core::option::Option::Some(()) => {
                    return Ok(Err("flowchart DOM ordinals must be distinct"));
                },
                ::core::option::Option::None => {},
            }
        }

        control.checkpoint()?;
        Ok(Ok(()))
    }

    /// Freezes label, CSS, and collapse facts for the supplied canonical subgraph order.
    #[must_use]
    pub fn new(
        labels: FlowchartRenderLabelSources,
        styles: FlowchartSubgraphStyles,
        collapsed_subgraphs: FixedSet<String>,
        subgraphs: &[FlowSubgraph],
    ) -> Self {
        let collapsed_replacements = build_collapsed_replacements(&collapsed_subgraphs, subgraphs);
        Self {
            labels,
            styles,
            collapsed_subgraphs,
            collapsed_replacements,
            subgraph_color_ordinals: build_subgraph_color_ordinals(subgraphs),
            node_dom_indices: FixedMap::default(),
        }
    }

    /// Replaces the family-assigned DOM ordinals as one complete indexed data set.
    pub fn set_node_dom_indices(&mut self, indices: impl IntoIterator<Item = (String, usize)>) {
        self.node_dom_indices = indices.into_iter().collect();
    }

    /// Returns a parser-assigned DOM ordinal when the family owns node registration.
    #[doc(hidden)]
    #[must_use]
    pub fn node_dom_index(&self, id: &str) -> Option<usize> {
        self.node_dom_indices.get(id).copied()
    }

    /// Replaces the visible-container mapping with the family's complete frozen result.
    pub fn set_collapsed_replacements(
        &mut self,
        replacements: impl IntoIterator<Item = (String, String)>,
    ) {
        self.collapsed_replacements = replacements.into_iter().collect();
    }

    /// Replaces the palette ordinals with the family's complete frozen result.
    pub fn set_subgraph_color_ordinals(
        &mut self,
        ordinals: impl IntoIterator<Item = (String, usize)>,
    ) {
        self.subgraph_color_ordinals = ordinals.into_iter().collect();
    }

    /// Returns the container's palette ordinal, independent of model storage order.
    #[doc(hidden)]
    #[must_use]
    pub fn subgraph_color_ordinal(&self, id: &str) -> Option<usize> {
        self.subgraph_color_ordinals.get(id).copied()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn node_label_for_render<'a>(&'a self, node: &'a FlowNode) -> Option<&'a str> {
        self.labels.node_label_for_render(node)
    }

    #[doc(hidden)]
    /// Returns the source-faithful label for one edge at its semantic model ordinal.
    ///
    /// The ordinal is required because Mermaid permits duplicate edge ids.
    #[must_use]
    pub fn edge_label_for_render<'a>(
        &'a self,
        semantic_index: usize,
        edge: &'a FlowEdge,
    ) -> Option<&'a str> {
        self.labels.edge_label_for_render(semantic_index, edge)
    }

    #[doc(hidden)]
    #[must_use]
    pub fn subgraph_title_for_render<'a>(
        &'a self,
        declaration_ordinal: usize,
        subgraph: &'a FlowSubgraph,
    ) -> &'a str {
        self.labels
            .subgraph_title_for_render(declaration_ordinal, subgraph)
    }

    #[doc(hidden)]
    #[must_use]
    pub fn effective_subgraph_css<'a>(
        &'a self,
        declaration_ordinal: usize,
        subgraph: &'a FlowSubgraph,
    ) -> (&'a [String], &'a [String]) {
        self.styles
            .effective_subgraph_css(declaration_ordinal, subgraph)
    }

    #[doc(hidden)]
    #[must_use]
    pub fn contains_subgraph_vertex(&self, id: &str) -> bool {
        self.styles.contains_subgraph_vertex(id)
    }

    /// Returns whether the parser attached Mermaid's collapsed view to this subgraph.
    #[doc(hidden)]
    #[must_use]
    pub fn is_subgraph_collapsed(&self, id: &str) -> bool {
        self.collapsed_subgraphs.contains(id)
    }

    /// Returns the visible collapsed subgraph that replaces this node, when the node is hidden
    /// beneath a collapsed subgraph. The visible collapsed subgraph itself is not replaced.
    #[doc(hidden)]
    #[must_use]
    pub fn collapsed_replacement(&self, id: &str) -> Option<&str> {
        self.collapsed_replacements.get(id).map(String::as_str)
    }

    #[must_use]
    pub fn retained_bytes(&self) -> usize {
        self.labels
            .retained_bytes()
            .saturating_add(
                self.collapsed_subgraphs
                    .iter()
                    .map(String::len)
                    .sum::<usize>(),
            )
            .saturating_add(self.collapsed_replacements.iter().fold(
                0usize,
                |total, (id, replacement)| {
                    total
                        .saturating_add(id.len())
                        .saturating_add(replacement.len())
                },
            ))
            .saturating_add(
                self.subgraph_color_ordinals
                    .keys()
                    .chain(self.node_dom_indices.keys())
                    .fold(0usize, |total, id| {
                        total
                            .saturating_add(id.len())
                            .saturating_add(std::mem::size_of::<usize>())
                    }),
            )
    }
}

#[must_use]
fn build_subgraph_color_ordinals(subgraphs: &[FlowSubgraph]) -> FixedMap<String, usize> {
    let ids: FixedSet<&str> = subgraphs.iter().map(|graph| graph.id.as_str()).collect();
    let mut parents = FixedMap::default();

    for graph in subgraphs {
        for child in &graph.nodes {
            match {
                ::core::primitive::bool::then_some(ids.contains(child.as_str()), ())
            } {
                ::core::option::Option::Some(()) => {
                    parents.insert(child.as_str(), graph.id.as_str());
                },
                ::core::option::Option::None => {},
            }
        }
    }

    let mut children: FixedMap<&str, Vec<&str>> = FixedMap::default();

    for graph in subgraphs {
        {
            match parents.get(graph.id.as_str()) {
                Some(&parent) => {
                    children.entry(parent).or_default().push(graph.id.as_str());
                },
                ::core::option::Option::None => {},
            };
        }
    }

    // Jison stores containers when they close. A containment preorder restores declaration
    // order and keeps each slot stable when the same container is rendered collapsed.
    let mut pending: Vec<_> = subgraphs
        .iter()
        .rev()
        .filter(|graph| !parents.contains_key(graph.id.as_str()))
        .map(|graph| graph.id.as_str())
        .collect();
    let mut ordinals = FixedMap::default();

    while let Some(id) = pending.pop() {
        match {
            ::core::primitive::bool::then_some(ordinals.contains_key(id), ())
        } {
            ::core::option::Option::Some(()) => {
                continue;
            },
            ::core::option::Option::None => {},
        }

        ordinals.insert(id.to_owned(), ordinals.len());

        {
            match children.get(id) {
                Some(children) => {
                    pending.extend(children.iter().rev().copied());
                },
                ::core::option::Option::None => {},
            };
        }
    }

    ordinals
}

#[must_use]
fn build_collapsed_replacements(
    collapsed_subgraphs: &FixedSet<String>,
    subgraphs: &[FlowSubgraph],
) -> FixedMap<String, String> {
    let subgraph_ids: FixedSet<&str> = subgraphs.iter().map(|sg| sg.id.as_str()).collect();
    let mut parent_by_id: FixedMap<&str, &str> = FixedMap::default();

    for subgraph in subgraphs {
        for child in &subgraph.nodes {
            match {
                ::core::primitive::bool::then_some(subgraph_ids.contains(child.as_str()), ())
            } {
                ::core::option::Option::Some(()) => {
                    parent_by_id.insert(child.as_str(), subgraph.id.as_str());
                },
                ::core::option::Option::None => {},
            }
        }
    }

    #[must_use]
    fn outermost_collapsed<'a>(
        id: &'a str,
        collapsed_subgraphs: &FixedSet<String>,
        parent_by_id: &FixedMap<&'a str, &'a str>,
    ) -> Option<&'a str> {
        let mut current = Some(id);
        let mut result = None;
        let mut seen: FixedSet<&str> = FixedSet::default();

        while let Some(candidate) = current {
            match {
                ::core::primitive::bool::then_some(!seen.insert(candidate), ())
            } {
                ::core::option::Option::Some(()) => {
                    break;
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(collapsed_subgraphs.contains(candidate), ())
            } {
                ::core::option::Option::Some(()) => {
                    result = Some(candidate);
                },
                ::core::option::Option::None => {},
            }

            current = parent_by_id.get(candidate).copied();
        }

        result
    }

    let mut replacements = FixedMap::default();

    for subgraph in subgraphs {
        let Some(ancestor) =
            outermost_collapsed(subgraph.id.as_str(), collapsed_subgraphs, &parent_by_id)
        else {
            continue;
        };

        match {
            ::core::primitive::bool::then_some(subgraph.id != ancestor, ())
        } {
            ::core::option::Option::Some(()) => {
                replacements.insert(subgraph.id.clone(), ancestor.to_string());
            },
            ::core::option::Option::None => {},
        }

        for child in &subgraph.nodes {
            match {
                ::core::primitive::bool::then_some(child != ancestor, ())
            } {
                ::core::option::Option::Some(()) => {
                    replacements.insert(child.clone(), ancestor.to_string());
                },
                ::core::option::Option::None => {},
            }
        }
    }

    replacements
}

#[cfg(test)]
#[path = "presentation_tests.rs"]
mod tests;
