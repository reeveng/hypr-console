//! An invocation identifies a fork during one execution, including when its job key is reused.

use std::collections::{BTreeMap, btree_map::Entry};

use console_core_never::Never;
use console_program_contract::Task;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Invocation {
  pub job: Task,
  sequence: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyStatus {
  Accepted,
  Stale,
}

impl From<bool> for ReplyStatus {
  fn from(accepted: bool) -> Self {
    match bool::then_some(accepted, ()) {
      Some(()) => Self::Accepted,
      None => Self::Stale,
    }
  }
}

impl Invocation {
  fn issue(sequence: &mut u64, job: Task) -> Result<Self, StartError> {
    let checked_value = sequence.checked_add(1).ok_or(StartError::Exhausted)?;
    *sequence = checked_value;

    Ok(Invocation {
      job,
      sequence: *sequence,
    })
  }

  pub fn matches_reply(
    self,
    invocation: Invocation,
    replied: Option<Task>,
  ) -> Result<ReplyStatus, Never> {
    Ok(ReplyStatus::from(
      self == invocation && replied == Some(self.job),
    ))
  }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StartError {
  Active(Task),
  Exhausted,
}

#[derive(Default)]
pub(crate) struct Tasks {
  pub(crate) live: BTreeMap<Task, Invocation>,
  sequence: u64,
}

impl Tasks {
  pub(crate) fn start(&mut self, job: Task) -> Result<Invocation, StartError> {
    let entry = match self.live.entry(job) {
      Entry::Occupied(_) => return Err(StartError::Active(job)),
      Entry::Vacant(entry) => entry,
    };
    let invocation = Invocation::issue(&mut self.sequence, job)?;

    entry.insert(invocation);

    Ok(invocation)
  }

  pub(crate) fn complete_reply(
    &mut self,
    invocation: Invocation,
    replied: Option<Task>,
  ) -> Result<ReplyStatus, Never> {
    Ok(match self.live.entry(invocation.job) {
      Entry::Occupied(entry) => {
        let Ok(status) = entry.get().matches_reply(invocation, replied);

        match status {
          ReplyStatus::Accepted => {
            entry.remove();
          }
          ReplyStatus::Stale => {}
        }

        status
      }
      Entry::Vacant(_) => ReplyStatus::Stale,
    })
  }
}

#[cfg(kani)]
#[path = "../proofs/jobs.rs"]
mod proofs;
