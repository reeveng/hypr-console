//! A machine: a state, one request handled, the next state, and what the world
//! has to be asked to do, said as values on a queue whoever drives it owns.
//!
//! The words are effect's, from `@effect/experimental/Machine`: a machine is
//! `initialize`d from its input and, when it was saved, from the state it had
//! before; it `handle`s one request at a time; one that can be written down
//! and read back is a `SerializableMachine`; an `Actor` is one running on a
//! thread of its own, and that is `console-actor`. What is not effect's is
//! that nothing here performs anything. A handler in effect forks the work it
//! wants; a handler here offers an `Effect` to a `Queue`, and whatever drives the
//! machine carries it out. That is what lets a recorded run replay on a laptop
//! with no compositor and reach the state the device reached, and it is the
//! split `console-program-contract` was already written on.
//! Work that can block belongs in an effect. The driver delivers its outcome
//! as another request.
//! Every effect names the services its driver needs through
//! [`EffectDependencies`]. A transition can report the union of those services
//! before the driver carries it out.
//! Disable default features to use [`SingleEffectMachine`] without a heap.
//! Use [`SingleEffectMachine::run_effects`] when each effect has an immediate
//! reply. Pass a transition from initialization or from handling a request,
//! a maximum effect count, and an interpreter with its context. The
//! interpreter returns a request, which can report a failed operation. The
//! runner returns the settled state or a pending transition when the count is
//! exhausted. Pass that transition back to resume without repeating effects.
//! A driver that waits for external events can handle each turn itself.
//! Enable `alloc` for growable queues, snapshots, and transcripts on targets
//! with an allocator. The default `std` feature also enables Unix path snapshots.
//! A driver can drain a queue in order while retaining its storage. An initial
//! reservation is limited to 4096 entries. The queue can grow past that as
//! effects are offered.
//!
//! **A handler cannot fail.** A request that means nothing in the state it
//! arrives in is a transition back to the same state, written as an arm; a
//! fault met while carrying an effect out comes back as a request. An error a
//! handler could return would be a third outcome beside the state and the
//! effects, and the one nobody draws on the map.
//!
//! **The state is moved, not borrowed.** `handle` takes the state and gives one
//! back, so a large state is never copied to change a small part of it, and a
//! machine made of machines hands each child its own part and puts back what
//! the child returns. An earlier draft asked every state, request and effect to
//! be `Copy`, which nothing that holds a path, a name or a list can be.
//!
//! **Saving is a machine's to choose.** With `alloc`, a machine can implement
//! `SerializableMachine`. `initialize` is handed the state a
//! saved machine had and decides what of it to keep: what a person owns -- the
//! page they were on, the workspace an app was on, a half-typed line -- comes
//! back, and a reading of the world -- which panel was up, how full the battery
//! was -- is asked again, because a reading restored after a reboot is one that
//! is confidently wrong.
//!
//! The `state-machines.md` note in Orbit is the rest of the argument.

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "alloc")]
use alloc::{collections::BTreeSet, vec, vec::Vec};

mod dependency;
#[cfg(feature = "alloc")]
mod interleavings;
#[cfg(feature = "alloc")]
mod schema;
mod single;
#[cfg(feature = "alloc")]
mod snapshot;
#[cfg(feature = "alloc")]
mod transcript;

pub use dependency::{EffectDependencies, Service};
#[cfg(feature = "alloc")]
pub use interleavings::{ExplorationError, interleavings};
#[cfg(feature = "alloc")]
pub use schema::{Decoder, Encoder, ParseError, Serializable};
pub use single::{SingleEffectMachine, SingleTransition};
#[cfg(feature = "alloc")]
pub use snapshot::{SerializableMachine, Version, restore, snapshot};
#[cfg(feature = "alloc")]
pub use transcript::{Step, Trace, Transcript, run, run_from};

pub use console_core_never::Never;

#[cfg(feature = "alloc")]
pub trait Machine {
  type Input;
  type State;
  type Request;
  type Effect: EffectDependencies;

  fn initialize(
    input: &Self::Input,
    previous: Option<Self::State>,
    effects: &mut Queue<Self::Effect>,
  ) -> Result<Self::State, Never>;

  fn handle(
    state: Self::State,
    request: Self::Request,
    effects: &mut Queue<Self::Effect>,
  ) -> Result<Self::State, Never>;

  fn initial_transition(
    input: &Self::Input,
    previous: Option<Self::State>,
  ) -> Result<Transition<Self::State, Self::Effect>, Never> {
    let Ok(mut queued) = Queue::unbounded();
    let Ok(state) = Self::initialize(input, previous, &mut queued);
    let Ok(effects) = queued.take_all();

    Ok(Transition { state, effects })
  }

  fn transition(
    state: Self::State,
    request: Self::Request,
  ) -> Result<Transition<Self::State, Self::Effect>, Never> {
    let Ok(mut queued) = Queue::unbounded();
    let Ok(next) = Self::handle(state, request, &mut queued);
    let Ok(effects) = queued.take_all();

    Ok(Transition {
      state: next,
      effects,
    })
  }
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition<S, E> {
  pub state: S,
  pub effects: Vec<E>,
}

#[cfg(feature = "alloc")]
impl<S, E> Transition<S, E> {
  pub fn new(state: S, effects: Vec<E>) -> Result<Self, Never> {
    Ok(Transition { state, effects })
  }

  pub fn without_effects(state: S) -> Result<Self, Never> {
    Ok(Transition {
      state,
      effects: Vec::new(),
    })
  }

  pub fn offered(self, to: &mut Queue<E>) -> Result<S, Never> {
    let Transition { state, effects } = self;
    let Ok(()) = to.offer_all(effects);

    Ok(state)
  }
}

#[cfg(feature = "alloc")]
impl<S, E: EffectDependencies> Transition<S, E> {
  pub fn services(&self) -> Result<BTreeSet<Service>, Never> {
    Ok(
      self
        .effects
        .iter()
        .flat_map(|effect| {
          let Ok(services) = effect.services();
          services.iter().copied()
        })
        .collect(),
    )
  }
}

#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queue<T> {
  items: Vec<T>,
}

#[cfg(feature = "alloc")]
impl<T> Queue<T> {
  pub fn unbounded() -> Result<Self, Never> {
    Ok(Queue { items: Vec::new() })
  }

  pub fn with_capacity(capacity: u32) -> Result<Self, Never> {
    let Ok(requested) = console_core_number_conversion::index(capacity);

    Ok(Queue {
      items: Vec::with_capacity(requested.min(4096)),
    })
  }

  pub fn offer(&mut self, item: T) -> Result<(), Never> {
    self.items.push(item);

    Ok(())
  }

  pub fn offer_all(&mut self, items: impl IntoIterator<Item = T>) -> Result<(), Never> {
    self.items.extend(items);

    Ok(())
  }

  pub fn take_all(&mut self) -> Result<Vec<T>, Never> {
    Ok(core::mem::take(&mut self.items))
  }

  pub fn drain(&mut self) -> Result<vec::Drain<'_, T>, Never> {
    Ok(self.items.drain(..))
  }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
  use super::*;

  #[test]
  fn draining_preserves_effect_order_and_reserved_storage() {
    let Ok(mut queue) = Queue::with_capacity(2);
    let storage = queue.items.as_ptr();

    for effects in [[1, 2], [3, 4]] {
      let Ok(()) = queue.offer_all(effects);
      let Ok(drained) = queue.drain();

      assert!(drained.eq(effects));
      assert_eq!(queue.items.as_ptr(), storage);
      assert!(queue.items.is_empty());
    }
  }
}
