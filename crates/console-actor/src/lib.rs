//! A machine on a thread of its own, reached by sending it requests.
//!
//! effect's `Actor`, from the same module as its `Machine`: `boot` starts one,
//! `send` hands it a request and waits for what that request decided, and
//! `get` reads the state it holds. What two threads used to share behind a
//! lock -- a panel draws on one and reads its rows on another -- is given one
//! owner instead, so there is nothing to lock and the type of a request says
//! exactly what can reach it. This was `console-panel`'s `actor` first, and
//! every card that holds a state still reads the way it did there.
//!
//! **The effects go back to whoever sent the request.** The actor decides and
//! never carries anything out, because the one who asked is the one who knows
//! where an effect can be carried: a panel's are carried on GTK's thread, which
//! is not this one. An earlier draft let a request be sent and forgotten, and
//! its effects were queued where nobody read them -- a machine that decided and
//! was never obeyed.
//!
//! **A request a machine falls under costs the state and nothing else.** The
//! panic is caught, the machine is initialized again -- from where it was last
//! kept, when it is kept -- and the sender is told `MachineDefect` with the
//! effects that put the world back. The mailbox and every handle survive. On a
//! desktop where every piece already starts itself again when it dies, that is
//! the same promise one step smaller.
//!
//! **A mailbox holds nobody.** Every `send` and every `get` waits for its
//! answer, so a request never sat in the mailbox without a thread stopped
//! behind it, and the mailbox never held more than the threads waiting on it.
//! It was an unbounded channel all the same, which said the opposite in its
//! type. It is a rendezvous now: a request is handed over when the actor takes
//! it, so the bound it always had is the one it is written with, and a send
//! that does not wait would have to say what it does while the actor is busy
//! rather than inherit a queue nobody sized.
//!
//! Each reply channel holds one answer. A sender waits for that answer so the
//! effects of an applied request are returned to its caller; closing waits for
//! the final save and the actor's thread to end. An idle actor waits for its
//! next request or the scope's end message.
//!
//! An event loop can use [`boot_buffered`] and [`Actor::try_submit`]. Each
//! actor has a bounded mailbox and each accepted request reserves room in
//! the caller's [`Completions`] queue. The actor publishes its effects or
//! fault there with the caller's identifier. Rejected submissions give the
//! request back. The loop can check for ready events with `try_recv` or wait
//! on its inbox with `recv_timeout` while idle.
//!
//! **A kept machine is saved when nothing is waiting for it.** Not on every
//! request: a held stick sends hundreds a second, and a write synced to the
//! disk for each is a handheld spending its flash on the same few bytes. When
//! the mailbox runs dry the state is written, and only if it differs from what
//! was written last; `Scope::close` writes it once more on the way out.

use std::any::type_name;
use std::fmt;
use std::num::NonZeroU16;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::thread::JoinHandle;

use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_state_machine::{Machine, Queue, SerializableMachine, restore, snapshot};

mod events;
mod key_value_store;

pub use events::{Completion, Completions, SubmitError};
pub use key_value_store::{KeyValueStore, PlatformError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError<E> {
  Closed,
  MachineDefect(Vec<E>),
}

impl<E> fmt::Display for SendError<E> {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      SendError::Closed => to.write_str("the actor has ended"),
      SendError::MachineDefect(_) => {
        to.write_str("the machine fell handling the request and was started again")
      }
    }
  }
}

impl<E: fmt::Debug> std::error::Error for SendError<E> {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Closed;

impl fmt::Display for Closed {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    to.write_str("the actor has ended")
  }
}

impl std::error::Error for Closed {}

#[derive(Debug)]
pub enum BootError {
  Thread(std::io::ErrorKind),
  Store(PlatformError),
}

impl fmt::Display for BootError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      BootError::Thread(kind) => write!(to, "the actor's thread would not start: {kind}"),
      BootError::Store(fault) => write!(
        to,
        "what the actor saved last will not be read, so it was not started over it: {fault}"
      ),
    }
  }
}

impl std::error::Error for BootError {}

type Reply<E> = Result<Vec<E>, SendError<E>>;

enum ReplyTo<E> {
  Waiting(SyncSender<Reply<E>>),
  Event(events::EventReply<E>),
}

type Read<S> = Box<dyn FnOnce(&S) + Send>;

enum Envelope<M: Machine> {
  Send(M::Request, ReplyTo<M::Effect>),
  Read(Read<M::State>),
  End,
}

pub struct Actor<M: Machine> {
  mailbox: SyncSender<Envelope<M>>,
}

impl<M: Machine> Clone for Actor<M> {
  fn clone(&self) -> Self {
    Actor {
      mailbox: self.mailbox.clone(),
    }
  }
}

impl<M: Machine> Actor<M> {
  pub fn send(&self, request: M::Request) -> Result<Vec<M::Effect>, SendError<M::Effect>> {
    let (reply, replied) = sync_channel(1);

    match self
      .mailbox
      .send(Envelope::Send(request, ReplyTo::Waiting(reply)))
    {
      Ok(()) => {}
      Err(_ended) => return Err(SendError::Closed),
    }

    #[cfg_attr(
      dylint_lib = "explicit057_no_unbounded_wait",
      expect(
        explicit057_no_unbounded_wait,
        reason = "the actor owns the only reply sender and either handles the request, restarts after a panic, or drops the sender; a deadline would abandon effects of a request already applied"
      )
    )]
    let answer = replied.recv();

    match answer {
      Ok(outcome) => outcome,
      Err(_ended) => Err(SendError::Closed),
    }
  }

  pub fn get(&self) -> Result<M::State, Closed>
  where
    M::State: Clone + Send + 'static,
  {
    let (reply, replied) = sync_channel(1);
    let read: Read<M::State> =
      Box::new(move |state: &M::State| match reply.send(state.clone()) {
        Ok(()) => {}
        Err(_reader_gone) => eprintln!(
          "{}: the reader ended before its state was returned",
          type_name::<M>()
        ),
      });

    match self.mailbox.send(Envelope::Read(read)) {
      Ok(()) => {}
      Err(_ended) => return Err(Closed),
    }

    #[cfg_attr(
      dylint_lib = "explicit057_no_unbounded_wait",
      expect(
        explicit057_no_unbounded_wait,
        reason = "the actor owns the only reply sender and runs the read callback or drops it when it ends; the read shares the request mailbox's synchronous lifetime"
      )
    )]
    let answer = replied.recv();

    answer.map_err(|_ended| Closed)
  }
}

pub struct Scope {
  end: Box<dyn FnOnce() -> Result<(), Never> + Send>,
  thread: JoinHandle<()>,
}

impl Scope {
  pub fn close(self) -> Result<(), Never> {
    let Scope { end, thread } = self;
    let Ok(()) = end();

    #[cfg_attr(
      dylint_lib = "explicit057_no_unbounded_wait",
      expect(
        explicit057_no_unbounded_wait,
        reason = "the end message has been delivered and joining keeps close from returning before the actor saves its final state and releases its resources"
      )
    )]
    let ended = thread.join();

    match ended {
      Ok(()) => {}
      Err(_defect) => eprintln!("the actor's thread fell before closing finished"),
    }

    Ok(())
  }
}

pub struct Booted<M: Machine> {
  pub actor: Actor<M>,
  pub scope: Scope,
  pub effects: Vec<M::Effect>,
}

pub fn boot<M>(input: M::Input) -> Result<Booted<M>, BootError>
where
  M: Machine<
      Input: Send + 'static,
      State: Send + 'static,
      Request: Send + 'static,
      Effect: Send + 'static,
    > + 'static,
{
  start::<M, Forgets>(input, Forgets, 0)
}

pub fn boot_buffered<M>(
  input: M::Input,
  mailbox_capacity: NonZeroU16,
) -> Result<Booted<M>, BootError>
where
  M: Machine<
      Input: Send + 'static,
      State: Send + 'static,
      Request: Send + 'static,
      Effect: Send + 'static,
    > + 'static,
{
  start::<M, Forgets>(input, Forgets, mailbox_capacity.get())
}

pub fn boot_serializable<M>(
  input: M::Input,
  store: KeyValueStore,
  key: &str,
) -> Result<Booted<M>, BootError>
where
  M: SerializableMachine<
      Input: Send + 'static,
      State: Send + 'static,
      Request: Send + 'static,
      Effect: Send + 'static,
    > + 'static,
{
  let last = store.get(key).map_err(BootError::Store)?;

  start::<M, Saves>(
    input,
    Saves {
      store,
      key: key.to_string(),
      last,
    },
    0,
  )
}

fn start<M, K>(input: M::Input, keeper: K, mailbox_capacity: u16) -> Result<Booted<M>, BootError>
where
  M: Machine<
      Input: Send + 'static,
      State: Send + 'static,
      Request: Send + 'static,
      Effect: Send + 'static,
    > + 'static,
  K: Keeper<M>,
{
  let Ok(mut queue) = Queue::unbounded();
  let Ok(previous) = keeper.previous();
  let Ok(state) = M::initialize(&input, previous, &mut queue);
  let Ok(effects) = queue.take_all();
  let Ok(slots) = index(mailbox_capacity);
  let (mailbox, received) = sync_channel(slots);
  let serving = std::thread::Builder::new().spawn(move || {
    let Ok(()) = serve::<M, K>(Serving { input, keeper }, state, received);
  });
  let thread = match serving {
    Ok(thread) => thread,
    Err(fault) => return Err(BootError::Thread(fault.kind())),
  };
  let ending = mailbox.clone();
  let end = Box::new(move || {
    #[cfg_attr(
      dylint_lib = "explicit060_no_reasonless_discard",
      expect(
        explicit060_no_reasonless_discard,
        reason = "a disconnected mailbox means the actor has already ended; close still joins its thread and reports any panic"
      )
    )]
    let _ = ending.send(Envelope::End);

    Ok(())
  });

  Ok(Booted {
    actor: Actor { mailbox },
    scope: Scope { end, thread },
    effects,
  })
}

struct Serving<M: Machine, K> {
  input: M::Input,
  keeper: K,
}

fn serve<M: Machine, K: Keeper<M>>(
  mut serving: Serving<M, K>,
  initialized: M::State,
  received: Receiver<Envelope<M>>,
) -> Result<(), Never> {
  let mut state = initialized;

  for arrival in (Arrivals {
    received,
    since: Since::Handled,
  }) {
    state = match arrival {
      Arrival::Idle => {
        let Ok(()) = serving.keeper.idle(&state);

        state
      }
      Arrival::Send(request, reply) => {
        let Ok(next) = handled::<M, K>(&serving, state, request, reply);

        next
      }
      Arrival::Read(read) => {
        match catch_unwind(AssertUnwindSafe(|| read(&state))) {
          Ok(()) => {}
          Err(_defect) => eprintln!(
            "{}: the reader fell; the actor keeps its state",
            type_name::<M>()
          ),
        }

        state
      }
    };
  }

  serving.keeper.idle(&state)
}

fn handled<M: Machine, K: Keeper<M>>(
  serving: &Serving<M, K>,
  state: M::State,
  request: M::Request,
  reply: ReplyTo<M::Effect>,
) -> Result<M::State, Never> {
  let outcome = catch_unwind(AssertUnwindSafe(move || {
    let Ok(mut effects) = Queue::unbounded();
    let Ok(next) = M::handle(state, request, &mut effects);

    (next, effects)
  }));
  let (next, answer) = match outcome {
    Ok((next, mut effects)) => {
      let Ok(decided) = effects.take_all();

      (next, Ok(decided))
    }
    Err(defect) => {
      let reason = match (
        defect.downcast_ref::<&str>(),
        defect.downcast_ref::<String>(),
      ) {
        (Some(reason), _) => *reason,
        (None, Some(reason)) => reason.as_str(),
        (None, None) => "panic carried no text",
      };
      console_core_logging::error!(
        "{}: the machine fell handling a request and will restart from saved state: {reason}",
        type_name::<M>()
      );

      let Ok(mut effects) = Queue::unbounded();
      let Ok(previous) = serving.keeper.previous();
      let Ok(restarted) = M::initialize(&serving.input, previous, &mut effects);
      let Ok(decided) = effects.take_all();

      (restarted, Err(SendError::MachineDefect(decided)))
    }
  };

  match reply {
    ReplyTo::Waiting(waiting) => match waiting.send(answer) {
      Ok(()) => {}
      Err(_sender_gone) => eprintln!(
        "{}: the sender ended before its effects were returned",
        type_name::<M>()
      ),
    },
    ReplyTo::Event(completing) => {
      let Ok(()) = completing.finish(answer);
    }
  }

  Ok(next)
}

enum Since {
  Handled,
  Retained,
}

enum Arrival<M: Machine> {
  Idle,
  Send(M::Request, ReplyTo<M::Effect>),
  Read(Read<M::State>),
}

struct Arrivals<M: Machine> {
  received: Receiver<Envelope<M>>,
  since: Since,
}

impl<M: Machine> Iterator for Arrivals<M> {
  type Item = Arrival<M>;

  fn next(&mut self) -> Option<Arrival<M>> {
    let waited = match self.since {
      Since::Handled => self.received.try_recv(),
      Since::Retained => {
        #[cfg_attr(
          dylint_lib = "explicit057_no_unbounded_wait",
          expect(
            explicit057_no_unbounded_wait,
            reason = "an idle actor waits for a request or End from its owning scope; a timeout would terminate a live actor solely because nobody has sent it work"
          )
        )]
        let arrival = self.received.recv();

        arrival.map_err(|_ended| TryRecvError::Disconnected)
      }
    };

    match waited {
      Ok(Envelope::Send(request, reply)) => {
        self.since = Since::Handled;

        Some(Arrival::Send(request, reply))
      }
      Ok(Envelope::Read(read)) => Some(Arrival::Read(read)),
      Ok(Envelope::End) => None,
      Err(TryRecvError::Empty) => {
        self.since = Since::Retained;

        Some(Arrival::Idle)
      }
      Err(TryRecvError::Disconnected) => None,
    }
  }
}

trait Keeper<M: Machine>: Send + 'static {
  fn previous(&self) -> Result<Option<M::State>, Never>;

  fn idle(&mut self, state: &M::State) -> Result<(), Never>;
}

struct Forgets;

impl<M: Machine> Keeper<M> for Forgets {
  fn previous(&self) -> Result<Option<M::State>, Never> {
    Ok(None)
  }

  fn idle(&mut self, _state: &M::State) -> Result<(), Never> {
    Ok(())
  }
}

struct Saves {
  store: KeyValueStore,
  key: String,
  last: Option<Vec<u8>>,
}

impl<M: SerializableMachine> Keeper<M> for Saves {
  fn previous(&self) -> Result<Option<M::State>, Never> {
    Ok(match &self.last {
      None => None,
      Some(bytes) => match restore::<M>(bytes) {
        Ok(state) => Some(state),
        Err(fault) => {
          eprintln!(
            "{}: what was saved will not be read, so it starts fresh: {fault}",
            self.key
          );

          None
        }
      },
    })
  }

  fn idle(&mut self, state: &M::State) -> Result<(), Never> {
    let Ok(bytes) = snapshot::<M>(state);

    match ::core::primitive::bool::then_some(self.last.as_ref() == Some(&bytes), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => match self.store.set(&self.key, &bytes) {
        Ok(()) => self.last = Some(bytes),
        Err(fault) => eprintln!("{}: the state was not saved: {fault}", self.key),
      },
    }

    Ok(())
  }
}
