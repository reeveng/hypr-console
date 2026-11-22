//! The document root is held separately so every constructed tree has one.
//! Remaining nodes keep their document order in a contiguous allocation.

use crate::{Error, NodeData, NodeId};
use alloc::vec::Vec;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

pub(super) struct NodeList<'input> {
  pub(super) root: NodeData<'input>,
  rest: Vec<NodeData<'input>>,
}

impl<'input> NodeList<'input> {
  pub(super) fn new(root: NodeData<'input>, capacity: u64) -> Result<Self, Never> {
    let Ok(capacity_index) = index(capacity.saturating_sub(1));

    Ok(Self {
      root,
      rest: Vec::with_capacity(capacity_index.min(4_096)),
    })
  }

  pub(super) fn len(&self) -> Result<u64, Never> {
    let Ok(length) = fitted::<_, u64>(self.rest.len());

    Ok(length.saturating_add(1))
  }

  pub(super) fn push(&mut self, data: NodeData<'input>) -> Result<NodeId, Error> {
    let Ok(length) = self.len();
    let id = NodeId::from_count(length).map_err(|_outside_ids| Error::NodesLimitReached)?;

    self.rest.push(data);

    Ok(id)
  }

  pub(super) fn get(&self, id: NodeId) -> Result<Option<&NodeData<'input>>, Never> {
    let Ok(position) = id.get();

    Ok(match position {
      0 => Some(&self.root),
      other => {
        let Ok(at) = index(other.saturating_sub(1));

        self.rest.get(at)
      }
    })
  }

  pub(super) fn get_mut(&mut self, id: NodeId) -> Result<Option<&mut NodeData<'input>>, Never> {
    let Ok(position) = id.get();

    Ok(match position {
      0 => Some(&mut self.root),
      other => {
        let Ok(at) = index(other.saturating_sub(1));

        self.rest.get_mut(at)
      }
    })
  }

  pub(super) fn last_mut(&mut self) -> Result<&mut NodeData<'input>, Never> {
    Ok(match self.rest.last_mut() {
      Some(last) => last,
      None => &mut self.root,
    })
  }

  pub(super) fn shrink_to_fit(&mut self) -> Result<(), Never> {
    self.rest.shrink_to_fit();

    Ok(())
  }
}
