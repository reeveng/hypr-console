pub(crate) use merman_model::families::state::{
    StateDocument, StateDocumentId, StateStatement, StateStatementNote, StateStatementState,
};
use merman_model::families::state::{StateDocumentBuilder, StateDocumentError};

/// Mutable grammar/DB construction; only a validated completed tree crosses into model data.
#[derive(Debug, Default)]
pub(crate) struct StateDocumentScratch {
    builder: StateDocumentBuilder,
    pub(super) root: Option<StateDocumentId>,
}

impl StateDocumentScratch {
    pub(crate) fn push(&mut self, statements: Vec<StateStatement>) -> StateDocumentId {
        self.builder.append(statements)
    }

    #[must_use]
    pub(super) fn get(&self, id: StateDocumentId) -> Option<&[StateStatement]> {
        self.builder.get(id)
    }

    pub(super) fn get_mut(&mut self, id: StateDocumentId) -> &mut Vec<StateStatement> {
        self.builder
            .edit(id)
            .expect("scratch document id was allocated by this builder")
    }

    #[must_use]
    pub(super) fn root_statements(&self) -> &[StateStatement] {
        match self.root.and_then(|id| self.get(id)) {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => &[],
        }
    }

    pub(super) fn finish_controlled(
        self,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<Result<StateDocument, StateDocumentError>> {
        self.builder.build_controlled(self.root, control)
    }
}
