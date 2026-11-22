//! Canonical State data, model-owned flat documents, and optional full presentation data.

#[cfg(feature = "compat-json")]
pub mod compat;

/// Pinned Mermaid class for a transition joining a note.
pub const CSS_EDGE_NOTE_EDGE: &str = "transition note-edge";

mod ast;
mod complexity;
mod model;
#[cfg(feature = "presentation-data")]
mod render_model;
mod terminal;
#[cfg(test)]
mod tests;

pub use ast::{
    StateStatement, StateStatementClick, StateStatementNote, StateStatementRelation,
    StateStatementState,
};
pub use model::{
    StateDiagramModel, StateEdge, StateLink, StateLinks, StateNode, StateNote, StateRecord,
    StateRelation, StateStyleClass,
};
#[cfg(feature = "presentation-data")]
pub use render_model::{StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode};
pub use terminal::{
    StateTerminalEdge, StateTerminalEdges, StateTerminalNode, StateTerminalNodes, StateTerminalView,
};

/// Index of a document in its owning model's flat State tree.
///
/// Identifiers are issued by [`StateDocumentBuilder`], never constructed from raw offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateDocumentId(usize);

impl StateDocumentId {
    /// Stable flat-document ordinal used by borrowed serializers.
    #[must_use]
    pub fn index(self) -> usize {
        self.0
    }
}

/// Completed statements owned once, with validated child references and one root.
///
/// The document store is a tree. State records may share references into this tree without
/// duplicating statements. Cloning and disposal operate on flat storage at arbitrary depth.
#[derive(Debug, Clone, Default)]
pub struct StateDocument {
    documents: Vec<Vec<StateStatement>>,
    root: Option<StateDocumentId>,
}

impl StateDocument {
    #[must_use]
    pub fn root(&self) -> Option<StateDocumentId> {
        self.root
    }

    #[must_use]
    pub fn get(&self, id: StateDocumentId) -> Option<&[StateStatement]> {
        self.documents.get(id.0).map(Vec::as_slice)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Visits flat documents in their original allocation order.
    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (StateDocumentId, &[StateStatement])> {
        self.documents
            .iter()
            .enumerate()
            .map(|(index, statements)| (StateDocumentId(index), statements.as_slice()))
    }
}

/// Mutable construction storage, kept separate from a completed [`StateDocument`].
///
/// Source builders may append and rewrite whole statement documents (including concurrency
/// splits) before completion. Only [`Self::build`] or [`Self::build_controlled`] can establish
/// the final root; both validate every child reference, unique parentage, and reachability.
#[derive(Debug, Default)]
pub struct StateDocumentBuilder {
    documents: Vec<Vec<StateStatement>>,
}

impl StateDocumentBuilder {
    /// Appends one statement document and returns its model-owned identifier.
    pub fn append(&mut self, statements: Vec<StateStatement>) -> StateDocumentId {
        let id = StateDocumentId(self.documents.len());
        self.documents.push(statements);
        id
    }

    #[must_use]
    pub fn get(&self, id: StateDocumentId) -> Option<&[StateStatement]> {
        self.documents.get(id.0).map(Vec::as_slice)
    }

    /// Revises a construction document before its tree is validated and completed.
    pub fn edit(&mut self, id: StateDocumentId) -> Option<&mut Vec<StateStatement>> {
        self.documents.get_mut(id.0)
    }

    pub fn build(self, root: Option<StateDocumentId>) -> Result<StateDocument, StateDocumentError> {
        self.build_controlled(root, &crate::OperationControl::new())
            .expect("a private operation control cannot be cancelled")
    }

    /// Completes the tree with cooperative cancellation checks during validation.
    ///
    /// Cancellation is checked before inspecting the root and periodically while visiting
    /// documents and statements. A structural failure found between checkpoints is returned
    /// as the inner error; no partially validated document is ever published.
    pub fn build_controlled(
        self,
        root: Option<StateDocumentId>,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<Result<StateDocument, StateDocumentError>> {
        control.checkpoint()?;

        let Some(root) = root else {
            return Ok(match {
                ::core::primitive::bool::then_some(self.documents.is_empty(), ())
            } {
                ::core::option::Option::Some(()) => {
                    Ok(StateDocument::default())
                },
                ::core::option::Option::None => {
                    Err(StateDocumentError::InvalidRoot)
                },
            });
        };

        match {
            ::core::primitive::bool::then_some(root.0 >= self.documents.len(), ())
        } {
            ::core::option::Option::Some(()) => {
                return Ok(Err(StateDocumentError::InvalidRoot));
            },
            ::core::option::Option::None => {},
        }

        let mut seen = vec![false; self.documents.len()];
        let mut pending = vec![root];
        let mut steps = 0usize;

        while let Some(id) = pending.pop() {
            match {
                ::core::primitive::bool::then_some(steps.is_multiple_of(128), ())
            } {
                ::core::option::Option::Some(()) => {
                    control.checkpoint()?;
                },
                ::core::option::Option::None => {},
            }

            steps = steps.saturating_add(1);

            let Some(visited) = seen.get_mut(id.0) else {
                return Ok(Err(StateDocumentError::InvalidChild));
            };

            match {
                ::core::primitive::bool::then_some(*visited, ())
            } {
                ::core::option::Option::Some(()) => {
                    return Ok(Err(StateDocumentError::InvalidTree));
                },
                ::core::option::Option::None => {},
            }

            *visited = true;
            for statement in &self.documents[id.0] {
                match {
                    ::core::primitive::bool::then_some(steps.is_multiple_of(128), ())
                } {
                    ::core::option::Option::Some(()) => {
                        control.checkpoint()?;
                    },
                    ::core::option::Option::None => {},
                }

                steps = steps.saturating_add(1);

                match statement {
                    StateStatement::State(state) => {
                        match state.doc {
                            Some(child) => {
                                pending.push(child);
                            },
                            ::core::option::Option::None => {},
                        }
                    }
                    StateStatement::Relation(relation)
                        if relation.state1.doc.is_some() || relation.state2.doc.is_some() =>
                    {
                        // A document has one owning state statement, never a relation endpoint.
                        return Ok(Err(StateDocumentError::InvalidTree));
                    }
                    _ => {}
                }
            }
        }

        for (index, visited) in seen.into_iter().enumerate() {
            match {
                ::core::primitive::bool::then_some(index.is_multiple_of(128), ())
            } {
                ::core::option::Option::Some(()) => {
                    control.checkpoint()?;
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(!visited, ())
            } {
                ::core::option::Option::Some(()) => {
                    return Ok(Err(StateDocumentError::InvalidTree));
                },
                ::core::option::Option::None => {},
            }
        }

        control.checkpoint()?;
        Ok(Ok(StateDocument {
            documents: self.documents,
            root: Some(root),
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StateDocumentError {
    #[error("State document requires exactly one valid root")]
    InvalidRoot,
    #[error("State child document id does not belong to this store")]
    InvalidChild,
    #[error("State documents must form one rooted tree with a unique owning statement per child")]
    InvalidTree,
}
