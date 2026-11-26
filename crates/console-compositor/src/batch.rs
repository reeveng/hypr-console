//! Read related compositor observations in one request.

use std::fmt;
use std::path::Path;

use console_core_never::Never;

use crate::{HyprctlError, Query, Question, answer, socket};

#[derive(Debug)]
pub enum Error {
  Socket(socket::SocketError),
  Json(serde_json::Error),
  Question(HyprctlError),
  MissingReply(Query),
  ExtraReply,
}

impl fmt::Display for Error {
  fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Socket(fault) => fault.fmt(output),
      Self::Json(fault) => fault.fmt(output),
      Self::Question(fault) => fault.fmt(output),
      Self::MissingReply(query) => write!(output, "compositor batch omitted {query:?}"),
      Self::ExtraReply => output.write_str("compositor batch had an extra reply"),
    }
  }
}

impl std::error::Error for Error {}

pub fn ask_together<F: Question, S: Question>(
  first: F,
  second: S,
) -> Result<(F::Reply, S::Reply), Error> {
  let at = socket::socket(socket::Socket::Requests)
    .map_err(|fault| Error::Socket(socket::SocketError::Unplaced(fault)))?;
  let bytes = read_at(&at, &[F::ASKED, S::ASKED])?;

  decode(&bytes, |replies| {
    let a = next(replies, first)?;
    let b = next(replies, second)?;

    Ok((a, b))
  })
}

fn line(questions: &[Query]) -> Result<String, Never> {
  let commands: Vec<_> = questions
    .iter()
    .map(|query| {
      let Ok(command) = query.line();

      command
    })
    .collect();

  Ok(format!("[[BATCH]]{}", commands.join(";")))
}

pub(crate) fn read_at(at: &Path, questions: &[Query]) -> Result<Vec<u8>, Error> {
  let Ok(request) = line(questions);

  socket::send_at(at, &request).map_err(Error::Socket)
}

type Replies<'a> =
  serde_json::StreamDeserializer<'a, serde_json::de::StrRead<'a>, serde_json::Value>;

pub(crate) fn decode<T>(
  bytes: &[u8],
  read: impl FnOnce(&mut Replies<'_>) -> Result<T, Error>,
) -> Result<T, Error> {
  let Ok(text) = answer::text(bytes);
  let mut replies = serde_json::Deserializer::from_str(&text).into_iter();
  let result = read(&mut replies)?;

  match replies.next() {
    None => Ok(result),
    Some(_) => Err(Error::ExtraReply),
  }
}

pub(crate) fn next<Q: Question>(
  replies: &mut Replies<'_>,
  question: Q,
) -> Result<Q::Reply, Error> {
  let reply = replies.next().ok_or(Error::MissingReply(Q::ASKED))?;
  let value = reply.map_err(Error::Json)?;

  crate::said_of(question, value).map_err(Error::Question)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{ActiveWorkspace, Clients};

  #[test]
  fn paired_questions_use_their_existing_wire_spellings_in_order() {
    let Ok(sent) = line(&[Query::EveryMonitor, Query::Layers]);

    assert_eq!(sent, "[[BATCH]]j/monitors all;j/layers");
  }

  #[test]
  fn paired_replies_are_typed_and_keep_their_order() -> Result<(), Error> {
    let (front, windows) = decode(br#"{"id":7,"name":"seven"} []"#, |replies| {
      let front = next(replies, ActiveWorkspace)?;
      let windows = next(replies, Clients)?;

      Ok((front, windows))
    })?;

    assert_eq!(front.map(|workspace| workspace.id), Some(7));
    assert!(windows.is_empty());

    Ok(())
  }
}
