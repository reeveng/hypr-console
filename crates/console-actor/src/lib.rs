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
//! **A kept machine is saved when nothing is waiting for it.** Not on every
//! request: a held stick sends hundreds a second, and a write synced to the
//! disk for each is a handheld spending its flash on the same few bytes. When
//! the mailbox runs dry the state is written, and only if it differs from what
//! was written last; `Scope::close` writes it once more on the way out.

use std::any::type_name;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::JoinHandle;

use console_core_never::Never;
use console_core_state_machine::{Machine, Queue, SerializableMachine, restore, snapshot};

mod key_value_store;

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
            SendError::MachineDefect(_) => to.write_str("the machine fell handling the request and was started again"),
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
            BootError::Store(fault) => write!(to, "what the actor saved last will not be read, so it was not started over it: {fault}"),
        }
    }
}

impl std::error::Error for BootError {}

enum Reply<E> {
    Handled(Vec<E>),
    Restarted(Vec<E>),
}

type Read<S> = Box<dyn FnOnce(&S) + Send>;

enum Envelope<M: Machine> {
    Send(M::Request, Sender<Reply<M::Effect>>),
    Read(Read<M::State>),
    End,
}

pub struct Actor<M: Machine> {
    mailbox: Sender<Envelope<M>>,
}

impl<M: Machine> Clone for Actor<M> {
    fn clone(&self) -> Self {
        Actor { mailbox: self.mailbox.clone() }
    }
}

impl<M: Machine> Actor<M> {
    pub fn send(&self, request: M::Request) -> Result<Vec<M::Effect>, SendError<M::Effect>> {
        let (reply, replied) = channel();

        match self.mailbox.send(Envelope::Send(request, reply)) {
            Ok(()) => {}
            Err(_ended) => return Err(SendError::Closed),
        }

        match replied.recv() {
            Ok(Reply::Handled(effects)) => Ok(effects),
            Ok(Reply::Restarted(effects)) => Err(SendError::MachineDefect(effects)),
            Err(_ended) => Err(SendError::Closed),
        }
    }

    pub fn get(&self) -> Result<M::State, Closed>
    where
        M::State: Clone + Send + 'static,
    {
        let (reply, replied) = channel();
        let read: Read<M::State> = Box::new(move |state: &M::State| {
            let _ = reply.send(state.clone());
        });

        match self.mailbox.send(Envelope::Read(read)) {
            Ok(()) => {}
            Err(_ended) => return Err(Closed),
        }

        replied.recv().map_err(|_ended| Closed)
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
        let _ = thread.join();

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
    M: Machine<Input: Send + 'static, State: Send + 'static, Request: Send + 'static, Effect: Send + 'static> + 'static,
{
    start::<M, Forgets>(input, Forgets)
}

pub fn boot_serializable<M>(input: M::Input, store: KeyValueStore, key: &str) -> Result<Booted<M>, BootError>
where
    M: SerializableMachine<Input: Send + 'static, State: Send + 'static, Request: Send + 'static, Effect: Send + 'static>
        + 'static,
{
    let last = store.get(key).map_err(BootError::Store)?;

    start::<M, Saves>(input, Saves { store, key: key.to_string(), last })
}

fn start<M, K>(input: M::Input, keeper: K) -> Result<Booted<M>, BootError>
where
    M: Machine<Input: Send + 'static, State: Send + 'static, Request: Send + 'static, Effect: Send + 'static> + 'static,
    K: Keeper<M>,
{
    let Ok(mut queue) = Queue::unbounded();
    let Ok(previous) = keeper.previous();
    let Ok(state) = M::initialize(&input, previous, &mut queue);
    let Ok(effects) = queue.take_all();
    let (mailbox, received) = channel();
    let serving = std::thread::Builder::new().spawn(move || {
        let Ok(()) = serve::<M, K>(Serving { input, keeper }, state, received);
    });
    let thread = match serving {
        Ok(thread) => thread,
        Err(fault) => return Err(BootError::Thread(fault.kind())),
    };
    let ending = mailbox.clone();
    let end = Box::new(move || {
        let _ = ending.send(Envelope::End);

        Ok(())
    });

    Ok(Booted { actor: Actor { mailbox }, scope: Scope { end, thread }, effects })
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

    for arrival in (Arrivals { received, since: Since::Handled }) {
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
                let _ = catch_unwind(AssertUnwindSafe(|| read(&state)));

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
    reply: Sender<Reply<M::Effect>>,
) -> Result<M::State, Never> {
    let outcome = catch_unwind(AssertUnwindSafe(move || {
        let Ok(mut effects) = Queue::unbounded();
        let Ok(next) = M::handle(state, request, &mut effects);

        (next, effects)
    }));
    let (next, answer) = match outcome {
        Ok((next, mut effects)) => {
            let Ok(decided) = effects.take_all();

            (next, Reply::Handled(decided))
        }
        Err(_defect) => {
            eprintln!("{}: the machine fell handling a request, so it starts again from where it was last kept", type_name::<M>());

            let Ok(mut effects) = Queue::unbounded();
            let Ok(previous) = serving.keeper.previous();
            let Ok(restarted) = M::initialize(&serving.input, previous, &mut effects);
            let Ok(decided) = effects.take_all();

            (restarted, Reply::Restarted(decided))
        }
    };
    let _ = reply.send(answer);

    Ok(next)
}

enum Since {
    Handled,
    Retained,
}

enum Arrival<M: Machine> {
    Idle,
    Send(M::Request, Sender<Reply<M::Effect>>),
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
            Since::Retained => self.received.recv().map_err(|_ended| TryRecvError::Disconnected),
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
                    eprintln!("{}: what was saved will not be read, so it starts fresh: {fault}", self.key);

                    None
                }
            },
        })
    }

    fn idle(&mut self, state: &M::State) -> Result<(), Never> {
        let Ok(bytes) = snapshot::<M>(state);

        match self.last.as_ref() == Some(&bytes) {
            true => {}
            false => match self.store.set(&self.key, &bytes) {
                Ok(()) => self.last = Some(bytes),
                Err(fault) => eprintln!("{}: the state was not saved: {fault}", self.key),
            },
        }

        Ok(())
    }
}
