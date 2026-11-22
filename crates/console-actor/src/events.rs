//! A caller-owned inbox bounds outstanding requests across actors. Accepted
//! requests occupy capacity until their completions are received. Each reply
//! owns one reserved slot and can publish a result or a closure outcome.

use std::fmt;
use std::num::NonZeroU16;
use std::sync::mpsc::{
  Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel,
};
use std::time::Duration;

use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_state_machine::Machine;

use crate::{Actor, Envelope, Reply, ReplyTo, SendError};

#[derive(Debug, PartialEq, Eq)]
pub struct Completion<Id, E> {
  pub id: Id,
  pub result: Result<Vec<E>, SendError<E>>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SubmitError<R> {
  MailboxFull(R),
  CompletionsFull(R),
  Closed(R),
}

impl<R> fmt::Display for SubmitError<R> {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::MailboxFull(_) => to.write_str("the actor's mailbox is full"),
      Self::CompletionsFull(_) => to.write_str("the caller's completion inbox is full"),
      Self::Closed(_) => to.write_str("the actor has ended"),
    }
  }
}

impl<R: fmt::Debug> std::error::Error for SubmitError<R> {}

pub struct Completions<Id, E> {
  send: SyncSender<Completion<Id, E>>,
  receive: Receiver<Completion<Id, E>>,
  available: u16,
}

impl<Id, E> Completions<Id, E> {
  pub fn new(capacity: NonZeroU16) -> Result<Self, Never> {
    let Ok(slots) = index(capacity.get());
    let (send, receive) = sync_channel(slots);

    Ok(Self {
      send,
      receive,
      available: capacity.get(),
    })
  }

  pub fn try_recv(&mut self) -> Result<Completion<Id, E>, TryRecvError> {
    let completion = self.receive.try_recv()?;
    self.available = self.available.saturating_add(1);

    Ok(completion)
  }

  pub fn recv_timeout(
    &mut self,
    timeout: Duration,
  ) -> Result<Completion<Id, E>, RecvTimeoutError> {
    let completion = self.receive.recv_timeout(timeout)?;
    self.available = self.available.saturating_add(1);

    Ok(completion)
  }
}

type Deliver<E> = Box<dyn FnOnce(Reply<E>) + Send>;

pub(crate) struct EventReply<E> {
  deliver: Option<Deliver<E>>,
}

impl<E> EventReply<E> {
  pub(crate) fn finish(mut self, result: Reply<E>) -> Result<(), Never> {
    match self.deliver.take() {
      Some(deliver) => deliver(result),
      None => {}
    }

    Ok(())
  }
}

impl<E> Drop for EventReply<E> {
  fn drop(&mut self) {
    match self.deliver.take() {
      Some(deliver) => deliver(Err(SendError::Closed)),
      None => {}
    }
  }
}

impl<M: Machine<Effect: Send + 'static>> Actor<M> {
  pub fn try_submit<Id: Copy + Send + 'static>(
    &self,
    completions: &mut Completions<Id, M::Effect>,
    id: Id,
    request: M::Request,
  ) -> Result<(), SubmitError<M::Request>> {
    match completions.available {
      0 => return Err(SubmitError::CompletionsFull(request)),
      _ => {}
    }

    let send = completions.send.clone();
    let deliver = Box::new(
      move |result| match send.try_send(Completion { id, result }) {
        Ok(()) => {}
        Err(TrySendError::Disconnected(completion)) => drop(completion),
        Err(TrySendError::Full(_)) => {
          eprintln!("the actor's reserved completion queue was full")
        }
      },
    );
    let reply = EventReply {
      deliver: Some(deliver),
    };

    match self
      .mailbox
      .try_send(Envelope::Send(request, ReplyTo::Event(reply)))
    {
      Ok(()) => {
        completions.available = completions.available.saturating_sub(1);

        Ok(())
      }
      Err(rejected) => rejected_submission(rejected),
    }
  }
}

fn rejected_submission<M: Machine>(
  rejected: TrySendError<Envelope<M>>,
) -> Result<(), SubmitError<M::Request>> {
  type Reject<R> = fn(R) -> SubmitError<R>;

  let (envelope, error): (_, Reject<M::Request>) = match rejected {
    TrySendError::Full(envelope) => (envelope, SubmitError::MailboxFull),
    TrySendError::Disconnected(envelope) => (envelope, SubmitError::Closed),
  };

  match envelope {
    Envelope::Send(request, ReplyTo::Event(mut reply)) => {
      reply.deliver = None;

      Err(error(request))
    }
    #[cfg_attr(
      dylint_lib = "explicit004_no_panic",
      expect(
        explicit004_no_panic,
        reason = "try_send returns the same envelope that try_submit handed it, which is always an event request"
      )
    )]
    Envelope::Send(_, ReplyTo::Waiting(_)) | Envelope::Read(_) | Envelope::End => {
      unreachable!("try_submit only sends event requests")
    }
  }
}
