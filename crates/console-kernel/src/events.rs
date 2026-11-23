//! Fixed-capacity event and effect primitives for a serialized kernel driver.
//! The driver supplies event and sender IDs from its routing context and
//! retains operation slots across reuse. The input queue replaces only the
//! newest snapshot so older snapshots and key edges keep their order.

#![forbid(unsafe_code)]

use console_core_never::Never;
use console_kernel_handles::{Bytes, ProcessId};

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "head and len index the same Rust array as its const generic capacity"
  )
)]
struct Queue<T, const N: usize> {
  slots: [Option<T>; N],
  head: usize,
  len: usize,
}

impl<T: Copy, const N: usize> Queue<T, N> {
  const EMPTY: Self = Self {
    slots: [None; N],
    head: 0,
    len: 0,
  };

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the offset, capacity and resulting slot are Rust array indices"
    )
  )]
  fn slot(&self, offset: usize) -> Result<usize, Never> {
    let before_wrap = N.saturating_sub(self.head);

    Ok(
      match ::core::primitive::bool::then_some(offset < before_wrap, ()) {
        ::core::option::Option::Some(()) => self.head.saturating_add(offset),
        ::core::option::Option::None => offset.saturating_sub(before_wrap),
      },
    )
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "free capacity is measured using the const generic Rust array length"
    )
  )]
  fn free(&self) -> Result<usize, Never> {
    Ok(N.saturating_sub(self.len))
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the slot returned by the queue is consumed by the Rust array API"
    )
  )]
  fn push_reserved(&mut self, item: T) -> Result<(), Never> {
    let Ok(tail) = self.slot(self.len);

    match self.slots.get_mut(tail) {
      Some(slot) => {
        *slot = Some(item);
        self.len = self.len.saturating_add(1);
      }
      None => {}
    }

    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "free capacity is checked against the Rust array index range"
    )
  )]
  fn push(&mut self, item: T) -> Result<(), T> {
    let Ok(free) = self.free();

    match free {
      0 => Err(item),
      _ => {
        let Ok(()) = self.push_reserved(item);

        Ok(())
      }
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the last occupied slot is a Rust array index"
    )
  )]
  fn tail_mut(&mut self) -> Result<Option<&mut T>, Never> {
    let last = match self.len.checked_sub(1) {
      Some(offset) => offset,
      None => return Ok(None),
    };
    let Ok(slot) = self.slot(last);

    Ok(self.slots.get_mut(slot).and_then(Option::as_mut))
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the next queue head is an index in its backing Rust array"
    )
  )]
  fn pop(&mut self) -> Result<Option<T>, Never> {
    match self.len {
      0 => return Ok(None),
      _ => {}
    }

    let first = self.slots.get_mut(self.head).and_then(Option::take);
    let Ok(next_head) = self.slot(1);
    self.head = next_head;
    self.len = self.len.saturating_sub(1);

    Ok(first)
  }
}

#[derive(Debug, PartialEq, Eq)]
pub struct EffectCredits(pub u32);

pub struct Proposal<'a, S, E> {
  pub next: S,
  pub effects: &'a [E],
}

#[derive(Debug, PartialEq, Eq)]
pub enum Backpressure {
  Credits,
  Queue,
}

pub struct Transitions<S, E, const N: usize> {
  state: S,
  queue: Queue<E, N>,
}

impl<S: Copy, E: Copy, const N: usize> Transitions<S, E, N> {
  pub fn starting_with(state: S) -> Result<Self, Never> {
    Ok(Self {
      state,
      queue: Queue::EMPTY,
    })
  }

  pub fn state(&self) -> Result<&S, Never> {
    Ok(&self.state)
  }

  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the source slice length is compared with the destination array capacity"
    )
  )]
  pub fn commit(
    &mut self,
    proposal: Proposal<'_, S, E>,
    credits: &mut EffectCredits,
  ) -> Result<(), Backpressure> {
    let cost = u32::try_from(proposal.effects.len()).map_err(|_wide| Backpressure::Credits)?;
    let remaining = credits.0.checked_sub(cost).ok_or(Backpressure::Credits)?;

    let Ok(free) = self.queue.free();

    match ::core::primitive::bool::then_some(free < proposal.effects.len(), ()) {
      ::core::option::Option::Some(()) => return Err(Backpressure::Queue),
      ::core::option::Option::None => {}
    }

    for effect in proposal.effects {
      let Ok(()) = self.queue.push_reserved(*effect);
    }

    self.state = proposal.next;
    credits.0 = remaining;

    Ok(())
  }

  pub fn next_effect(&mut self) -> Result<Option<E>, Never> {
    self.queue.pop()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
  Kernel,
  Process(ProcessId),
  Device(DeviceId),
}

pub struct Envelope<T> {
  id: EventId,
  source: Source,
  payload: T,
}

impl<T> Envelope<T> {
  pub fn from_process(id: EventId, sender: ProcessId, payload: T) -> Result<Self, Never> {
    Self::from_context(id, Source::Process(sender), payload)
  }

  pub fn from_context(id: EventId, sender: Source, payload: T) -> Result<Self, Never> {
    Ok(Self {
      id,
      source: sender,
      payload,
    })
  }

  pub fn id(&self) -> Result<EventId, Never> {
    Ok(self.id)
  }

  pub fn source(&self) -> Result<Source, Never> {
    Ok(self.source)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Generation(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Correlation {
  pub operation: OperationId,
  pub target: TargetId,
  pub generation: Generation,
}

pub struct Begin {
  pub operation: OperationId,
  pub target: TargetId,
  pub replies_from: Source,
  pub reserved: Bytes,
}

struct Pending {
  correlation: Correlation,
  replies_from: Source,
  reserved: Bytes,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BeginError {
  Busy,
  Exhausted,
}

pub struct Completion<T> {
  pub correlation: Correlation,
  pub value: T,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Settlement<T> {
  Ignored,
  Complete { value: T, refund: Bytes },
}

pub struct OperationSlot {
  generation: Generation,
  pending: Option<Pending>,
}

impl OperationSlot {
  pub const EMPTY: Self = Self {
    generation: Generation(0),
    pending: None,
  };

  pub fn begin(&mut self, request: Begin) -> Result<Correlation, BeginError> {
    match self.pending {
      Some(_) => return Err(BeginError::Busy),
      None => {}
    }

    let next = self
      .generation
      .0
      .checked_add(1)
      .ok_or(BeginError::Exhausted)?;
    let generation = Generation(next);
    let correlation = Correlation {
      operation: request.operation,
      target: request.target,
      generation,
    };
    self.generation = generation;
    self.pending = Some(Pending {
      correlation,
      replies_from: request.replies_from,
      reserved: request.reserved,
    });

    Ok(correlation)
  }

  pub fn complete<T>(&mut self, event: Envelope<Completion<T>>) -> Result<Settlement<T>, Never> {
    match self.pending.as_ref() {
      Some(pending) => {
        match ::core::primitive::bool::then_some(pending.replies_from == event.source, ()) {
          ::core::option::Option::Some(()) => self.settle(event.payload),
          ::core::option::Option::None => Ok(Settlement::Ignored),
        }
      }
      None => Ok(Settlement::Ignored),
    }
  }

  pub fn cancel(&mut self, correlation: Correlation) -> Result<Settlement<()>, Never> {
    self.settle(Completion {
      correlation,
      value: (),
    })
  }

  fn settle<T>(&mut self, completion: Completion<T>) -> Result<Settlement<T>, Never> {
    match self.pending.as_ref() {
      Some(pending) => match ::core::primitive::bool::then_some(
        pending.correlation == completion.correlation,
        (),
      ) {
        ::core::option::Option::Some(()) => {
          let refund = pending.reserved;
          self.pending = None;

          Ok(Settlement::Complete {
            value: completion.value,
            refund,
          })
        }
        ::core::option::Option::None => Ok(Settlement::Ignored),
      },
      None => Ok(Settlement::Ignored),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sequence(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEdge {
  Press,
  Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
  Key {
    code: u16,
    edge: KeyEdge,
  },
  Position {
    device: DeviceId,
    sequence: Sequence,
    x: i32,
    y: i32,
  },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputEvent {
  pub id: EventId,
  pub input: Input,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Admission {
  Enqueued,
  Replaced(EventId),
}

#[derive(Debug, PartialEq, Eq)]
pub enum InputFault {
  Backpressure(InputEvent),
  Stale(InputEvent),
}

pub struct InputQueue<const N: usize>(Queue<InputEvent, N>);

impl<const N: usize> InputQueue<N> {
  pub const EMPTY: Self = Self(Queue::EMPTY);

  pub fn offer(&mut self, event: InputEvent) -> Result<Admission, InputFault> {
    let Ok(tail) = self.0.tail_mut();

    match tail {
      Some(previous) => {
        let Ok(action) = replacement(previous.input, event.input);

        match action {
          Replacement::Append => {}
          Replacement::Newer => {
            let superseded = previous.id;
            *previous = event;

            return Ok(Admission::Replaced(superseded));
          }
          Replacement::Stale => return Err(InputFault::Stale(event)),
        }
      }
      None => {}
    }

    self.0.push(event).map_err(InputFault::Backpressure)?;

    Ok(Admission::Enqueued)
  }

  pub fn take(&mut self) -> Result<Option<InputEvent>, Never> {
    self.0.pop()
  }
}

enum Replacement {
  Append,
  Newer,
  Stale,
}

fn replacement(previous: Input, next: Input) -> Result<Replacement, Never> {
  Ok(match (previous, next) {
    (
      Input::Position {
        device, sequence, ..
      },
      Input::Position {
        device: incoming,
        sequence: incoming_sequence,
        ..
      },
    ) => match (device == incoming, incoming_sequence.0 > sequence.0) {
      (true, true) => Replacement::Newer,
      (true, false) => Replacement::Stale,
      (false, _) => Replacement::Append,
    },
    (Input::Position { .. }, Input::Key { .. })
    | (Input::Key { .. }, Input::Position { .. } | Input::Key { .. }) => Replacement::Append,
  })
}

#[cfg(kani)]
#[path = "../proofs/events.rs"]
mod proofs;

#[cfg(test)]
mod tests {
  use super::*;
  use console_kernel_handles::{Boot, HandleError, Kernel};

  #[test]
  fn a_process_payload_cannot_override_its_kernel_sender() -> Result<(), HandleError> {
    let Boot {
      mut kernel,
      root,
      root_job,
      ..
    } = Kernel::boot(crate::HOSTED_MEMORY)?;
    let browser = kernel.create_process(root, root_job)?;
    let id = EventId(1);
    let Ok(event) = Envelope::from_process(id, browser.id, root);
    let Ok(sender) = event.source();
    let Ok(stamped_id) = event.id();

    assert_eq!(sender, Source::Process(browser.id));
    assert_eq!(stamped_id, id);
    assert_eq!(event.payload, root);

    Ok(())
  }
}
