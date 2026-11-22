//! A reply, matched to the call that asked for it.
//!
//! A connection that makes calls hears the answers in whatever order they
//! come, on the same socket as the calls other programs make to it and the
//! signals the bus sends it unasked. One number ties a reply to its call: the
//! serial the call went out under, which the reply carries as the serial it
//! answers. So the caller says what each call was for when it sends it -- a
//! tag of its own -- and everything that arrives is read back as one of five
//! things: the reply to a call, the error a call was answered with, a call
//! from someone else, a signal, or a reply nobody here is waiting for.
//!
//! It holds serials and reads nothing off the socket. What arrived is handed
//! in, which is what lets a conversation with a service be pressed in a test
//! with no bus anywhere near it, and a reply is matched once: the second copy
//! of an answer is unmatched rather than a second answer.

use std::collections::BTreeMap;

use console_core_never::Never;

use crate::messages::{Kind, Message};

#[derive(Debug, Clone, PartialEq)]
pub enum Received<Tag> {
  Reply { to: Tag, reply: Message },
  Error { to: Tag, error: Message },
  Call(Message),
  Signal(Message),
  Unmatched(Message),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending<Tag> {
  waiting: BTreeMap<u32, Tag>,
}

impl<Tag> Pending<Tag> {
  pub fn new() -> Result<Pending<Tag>, Never> {
    Ok(Pending {
      waiting: BTreeMap::new(),
    })
  }

  pub fn sent(&mut self, serial: u32, tag: Tag) -> Result<(), Never> {
    self.waiting.insert(serial, tag);

    Ok(())
  }

  pub fn received(&mut self, message: Message) -> Result<Received<Tag>, Never> {
    Ok(match message.kind {
      Kind::Call => Received::Call(message),
      Kind::Signal => Received::Signal(message),
      Kind::Answer => {
        let Ok(answered) = self.answered(&message);

        match answered {
          Some(to) => Received::Reply { to, reply: message },
          None => Received::Unmatched(message),
        }
      }
      Kind::ErrorReply => {
        let Ok(answered) = self.answered(&message);

        match answered {
          Some(to) => Received::Error { to, error: message },
          None => Received::Unmatched(message),
        }
      }
    })
  }

  fn answered(&mut self, message: &Message) -> Result<Option<Tag>, Never> {
    Ok(match message.reply_to {
      Some(serial) => self.waiting.remove(&serial),
      None => None,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use crate::messages::{ErrorName, ValidationError, Whom};

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  enum Method {
    Pair,
    Connect,
  }

  fn call(serial: u32) -> Result<Message, Never> {
    let Ok(mut asked) = Message::call(&Whom {
      to: "org.bluez",
      at: "/org/bluez",
      on: "org.bluez.Device1",
      calling: "Pair",
    });

    asked.serial = serial;

    Ok(asked)
  }

  #[test]
  fn a_reply_comes_back_with_the_tag_its_call_went_out_under() {
    let Ok(mut pending) = Pending::new();
    let Ok(paired) = call(7);
    let Ok(connected) = call(8);
    let Ok(reply) = paired.answering();
    let Ok(error) = connected.failing(ErrorName("org.bluez.Error.Failed"), "no");

    let Ok(()) = pending.sent(7, Method::Pair);
    let Ok(()) = pending.sent(8, Method::Connect);

    assert_eq!(
      pending.received(error.clone()),
      Ok(Received::Error {
        to: Method::Connect,
        error
      })
    );
    assert_eq!(
      pending.received(reply.clone()),
      Ok(Received::Reply {
        to: Method::Pair,
        reply
      })
    );
  }

  #[test]
  fn a_reply_is_matched_once_and_a_second_copy_is_nobody_s() {
    let Ok(mut pending) = Pending::new();
    let Ok(paired) = call(3);
    let Ok(reply) = paired.answering();

    let Ok(()) = pending.sent(3, Method::Pair);
    let Ok(_first) = pending.received(reply.clone());

    assert_eq!(
      pending.received(reply.clone()),
      Ok(Received::Unmatched(reply))
    );
  }

  #[test]
  fn a_call_from_someone_else_is_a_call_even_when_its_serial_is_one_of_ours() {
    let Ok(mut pending) = Pending::new();
    let Ok(theirs) = call(5);
    let Ok(complaint) = theirs.complaining(ValidationError::UnknownMethod, "nothing here");

    let Ok(()) = pending.sent(5, Method::Pair);

    assert_eq!(pending.received(theirs.clone()), Ok(Received::Call(theirs)));
    assert_eq!(
      pending.received(complaint.clone()),
      Ok(Received::Error {
        to: Method::Pair,
        error: complaint
      })
    );
  }
}
