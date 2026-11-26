//! The compositor observations shared by the bar and overview.

use std::path::Path;
use std::time::{Duration, Instant};

use console_core_number_conversion::fixed_index;
use serde::{Deserialize, Serialize};

use crate::{
  ActiveWorkspace, Clients, Monitor, Monitors, Query, Question, Window, Workspace, Workspaces,
  answer, batch, socket,
};

pub use crate::batch::Error;

const QUESTIONS: [Query; 4] = [
  ActiveWorkspace::ASKED,
  Workspaces::ASKED,
  Clients::ASKED,
  Monitors::ASKED,
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(
  unknown_lints,
  reason = "the input rule checks serde before cfg_attr expands"
)]
#[expect(
  explicit064_no_unvalidated_input,
  reason = "the versioned event client limits the complete snapshot frame before deserializing these lists"
)]
pub struct Snapshot {
  pub front: Option<Workspace>,
  pub workspaces: Vec<Workspace>,
  pub clients: Vec<Window>,
  pub monitors: Vec<Monitor>,
}

#[derive(Debug, Default)]
pub struct Timing {
  pub socket: Duration,
  pub decode: Duration,
  pub requests: u32,
  pub bytes: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum QueryPlan {
  Batch,
  Separate,
}

pub fn capture_with(plan: QueryPlan) -> Result<Snapshot, Error> {
  let at = socket::socket(socket::Socket::Requests)
    .map_err(|fault| Error::Socket(socket::SocketError::Unplaced(fault)))?;

  capture_at(&at, plan).map(|(snapshot, _timing)| snapshot)
}

#[cfg_attr(
  dylint_lib = "explicit039_no_reading_the_clock",
  expect(
    explicit039_no_reading_the_clock,
    reason = "these observations measure the socket and decoder at the I/O boundary"
  )
)]
pub fn capture_at(at: &Path, plan: QueryPlan) -> Result<(Snapshot, Timing), Error> {
  match plan {
    QueryPlan::Batch => {
      let began = Instant::now();
      let bytes = batch::read_at(at, &QUESTIONS)?;
      let socket = began.elapsed();
      let decoding = Instant::now();
      let snapshot = decode_batch(&bytes)?;
      let Ok(reply_bytes) = fixed_index(bytes.len());
      let timing = Timing {
        socket,
        decode: decoding.elapsed(),
        requests: 1,
        bytes: reply_bytes,
      };

      Ok((snapshot, timing))
    }
    QueryPlan::Separate => {
      let mut timing = Timing::default();
      let front = separate(at, ActiveWorkspace, &mut timing)?;
      let workspaces = separate(at, Workspaces, &mut timing)?;
      let clients = separate(at, Clients, &mut timing)?;
      let monitors = separate(at, Monitors, &mut timing)?;
      let snapshot = Snapshot {
        front,
        workspaces,
        clients,
        monitors,
      };

      Ok((snapshot, timing))
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit039_no_reading_the_clock",
  expect(
    explicit039_no_reading_the_clock,
    reason = "each direct query is timed at its socket and decoder boundaries"
  )
)]
fn separate<Q: Question>(at: &Path, question: Q, timing: &mut Timing) -> Result<Q::Reply, Error> {
  let Ok(line) = Q::ASKED.line();
  let began = Instant::now();
  let bytes = socket::send_at(at, &line).map_err(Error::Socket)?;
  timing.socket = timing.socket.saturating_add(began.elapsed());
  timing.requests = timing.requests.saturating_add(1);
  let Ok(reply_bytes) = fixed_index(bytes.len());
  timing.bytes = timing.bytes.saturating_add(reply_bytes);

  let decoding = Instant::now();
  let Ok(text) = answer::text(&bytes);
  let reply = crate::read(question, &text).map_err(Error::Question)?;
  timing.decode = timing.decode.saturating_add(decoding.elapsed());

  Ok(reply)
}

pub fn decode_batch(bytes: &[u8]) -> Result<Snapshot, Error> {
  batch::decode(bytes, |replies| {
    let front = batch::next(replies, ActiveWorkspace)?;
    let workspaces = batch::next(replies, Workspaces)?;
    let clients = batch::next(replies, Clients)?;
    let monitors = batch::next(replies, Monitors)?;

    Ok(Snapshot {
      front,
      workspaces,
      clients,
      monitors,
    })
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn batch_reads_one_reply_per_question_in_order() -> Result<(), Error> {
    let bytes = br#"{"id":3,"name":"three","windows":0}

    [{"id":3,"name":"three","windows":0}]

    []

    [{"name":"eDP-1"}]"#;

    let snapshot = decode_batch(bytes)?;
    assert_eq!(snapshot.front.as_ref().map(|front| front.id), Some(3));
    assert_eq!(snapshot.workspaces.len(), 1);
    assert!(snapshot.clients.is_empty());
    assert_eq!(
      snapshot
        .monitors
        .first()
        .map(|monitor| monitor.named.as_str()),
      Some("eDP-1")
    );
    let empty = br#"{"id":0,"name":""} [] []"#;
    assert!(matches!(
      decode_batch(empty),
      Err(Error::MissingReply(crate::Query::Monitors))
    ));
    assert!(matches!(
      decode_batch(br#"{"id":0,"name":""} [] [] [] []"#),
      Err(Error::ExtraReply)
    ));

    Ok(())
  }

  #[test]
  fn a_non_utf8_window_title_survives_the_batch() -> Result<(), Error> {
    let bytes =
      b"{\"id\":2,\"name\":\"two\"} [] [{\"address\":\"0x23\",\"title\":\"caf\xe9\"}] []";
    let snapshot = decode_batch(bytes)?;
    assert_eq!(
      snapshot.clients.first().map(|client| client.title.as_str()),
      Some("café")
    );

    Ok(())
  }
}
