//! The session socket accepts one bounded JSON or raw PNG request per connection.

use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use console_core_bytes::Bytes;
use console_operating_system_linux::unix_stream::{Within, connect};
use serde::{Deserialize, Serialize};

use crate::history::{Capture, MAX_TEXT_BYTES, Preview};

const TIMEOUT: Duration = Duration::from_millis(250);
const PNG_TIME: Duration = Duration::from_secs(2);
const MAX_REQUEST: u32 = MAX_TEXT_BYTES.saturating_mul(6).saturating_add(128);
const MAX_REPLY: u32 = 24 * 1024;
pub(crate) const MAX_PASTE: u32 = 48 * 1024;

pub const MAX_PNG_REQUEST: u32 = 128 * 1024 * 1024;
pub(crate) const PEERS: u64 = 8;
pub(crate) const REQUEST_MEMORY: u64 = 135 * 1024 * 1024;
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

#[derive(Deserialize, Serialize)]
pub enum Request {
  List,
  ReadText,
  Restore(u64),
  CopyText(String),
  #[serde(skip)]
  CopyPng(Bytes),
  Clear,
  Pause(Capture),
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub enum Reply {
  List {
    paused: Capture,
    entries: Vec<Preview>,
  },
  #[serde(rename = "Done")]
  Finished,
  Text(String),
  Unavailable,
}

fn path() -> io::Result<PathBuf> {
  let Ok(runtime) = console_core_places::runtime();
  let directory = runtime.ok_or_else(|| io::Error::other("no session runtime directory"))?;

  Ok(directory.join("console-clipboard/history.sock"))
}

fn capture_path() -> io::Result<PathBuf> {
  let socket = path()?;

  Ok(socket.with_file_name("paused"))
}

pub fn capture() -> io::Result<Capture> {
  let flag = capture_path()?;

  match std::fs::metadata(flag) {
    Ok(_marker) => Ok(Capture::Paused),
    Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
      Some(()) => Ok(Capture::Recording),
      None => Err(fault),
    },
  }
}

pub fn set_capture(capture: Capture) -> io::Result<()> {
  let flag = capture_path()?;

  match capture {
    Capture::Paused => create_pause_marker(&flag),
    Capture::Recording => match std::fs::remove_file(flag) {
      Ok(()) => Ok(()),
      Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
        Some(()) => Ok(()),
        None => Err(fault),
      },
    },
  }
}

#[cfg_attr(
  dylint_lib = "explicit040_no_torn_write",
  expect(
    explicit040_no_torn_write,
    reason = "the pause marker is an empty file and exclusive creation commits its entire state in one operation"
  )
)]
fn create_pause_marker(flag: &Path) -> io::Result<()> {
  let created = OpenOptions::new()
    .write(true)
    .create_new(true)
    .mode(0o600)
    .open(flag);

  match created {
    Ok(_marker) => Ok(()),
    Err(fault) => match (fault.kind() == io::ErrorKind::AlreadyExists).then_some(()) {
      Some(()) => std::fs::metadata(flag).map(|_marker| ()),
      None => Err(fault),
    },
  }
}

pub fn listen() -> io::Result<UnixListener> {
  let socket = path()?;
  let parent = socket
    .parent()
    .ok_or_else(|| io::Error::other("no clipboard socket directory"))?;
  std::fs::create_dir_all(parent)?;
  let listener = bind(&socket)?;
  listener.set_nonblocking(true)?;

  Ok(listener)
}

fn bind(socket: &Path) -> io::Result<UnixListener> {
  match UnixListener::bind(socket) {
    Ok(listener) => Ok(listener),
    Err(fault) => match (fault.kind() == io::ErrorKind::AddrInUse).then_some(()) {
      Some(()) => rebind_stale(socket, fault),
      None => Err(fault),
    },
  }
}

fn rebind_stale(socket: &Path, bind_fault: io::Error) -> io::Result<UnixListener> {
  match connect(socket) {
    Ok(_active) => Err(bind_fault),
    Err(connect_fault) => {
      match (connect_fault.kind() == io::ErrorKind::ConnectionRefused).then_some(()) {
        Some(()) => {
          std::fs::remove_file(socket)?;
          UnixListener::bind(socket)
        }
        None => Err(bind_fault),
      }
    }
  }
}

fn read<T: serde::de::DeserializeOwned>(input: impl Read, maximum: u32) -> io::Result<T> {
  let bytes = console_core_bounded_reads::bytes(input, maximum)?;
  serde_json::from_slice(&bytes).map_err(io::Error::other)
}

fn read_png(input: impl Read) -> io::Result<Bytes> {
  let bytes = console_core_bounded_reads::payload(input, MAX_PNG_REQUEST)?;

  png(bytes)
}

fn png(bytes: Bytes) -> io::Result<Bytes> {
  match bytes.starts_with(PNG_SIGNATURE).then_some(()) {
    Some(()) => Ok(bytes),
    None => Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "expected a PNG image",
    )),
  }
}

fn decode_request(bytes: Bytes) -> io::Result<Request> {
  match bytes.as_ref() {
    [0x89, ..] => png(bytes).map(Request::CopyPng),
    _ => serde_json::from_slice(&bytes).map_err(io::Error::other),
  }
}

fn write(stream: &mut impl Write, message: &impl Serialize) -> io::Result<()> {
  let bytes = serde_json::to_vec(message)?;
  stream.write_all(&bytes)
}

fn finish_request(stream: &UnixStream, timed: Within<'_>) -> io::Result<Reply> {
  stream.shutdown(Shutdown::Write)?;
  read(timed, MAX_REPLY)
}

pub fn ask(request: Request) -> io::Result<Reply> {
  let socket = path()?;
  let stream = connect(&socket)?;
  let mut timed = Within::new(&stream, TIMEOUT)?;
  write(&mut timed, &request)?;
  finish_request(&stream, timed)
}

pub fn copy_png(input: impl Read) -> io::Result<Reply> {
  let png = read_png(input)?;
  let socket = path()?;
  let stream = connect(&socket)?;
  let mut timed = Within::new(&stream, PNG_TIME)?;
  timed.write_all(&png)?;
  finish_request(&stream, timed)
}

mod transfer;

pub(crate) use transfer::Peer;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn png_requests_keep_their_binary_bytes() -> io::Result<()> {
    let sent = b"\x89PNG\r\n\x1a\n\x00\xff";
    let request = decode_request(Bytes::from(sent.to_vec()))?;
    assert!(matches!(
      request,
      Request::CopyPng(bytes) if bytes.as_ref() == sent
    ));
    let invalid = decode_request(Bytes::from(b"\x89unrecognized".to_vec()));
    assert!(matches!(invalid, Err(fault) if fault.kind() == io::ErrorKind::InvalidData));
    let plain_text_request = decode_request(Bytes::from(br#"{"CopyText":"still text"}"#.to_vec()))?;
    assert!(matches!(
      plain_text_request,
      Request::CopyText(text) if text == "still text"
    ));

    Ok(())
  }

  #[test]
  fn a_finished_reply_keeps_its_wire_name() -> Result<(), serde_json::Error> {
    let encoded = serde_json::to_string(&Reply::Finished)?;
    let reply = serde_json::from_str(&encoded)?;

    assert_eq!(encoded, r#""Done""#);
    assert!(matches!(reply, Reply::Finished));

    Ok(())
  }

  #[test]
  fn clipboard_privacy_uses_the_existing_boolean_wire_format() -> Result<(), serde_json::Error> {
    let request = serde_json::from_str::<Request>(r#"{"Pause":true}"#)?;
    assert!(matches!(request, Request::Pause(Capture::Paused)));

    let reply = Reply::List {
      paused: Capture::Recording,
      entries: Vec::new(),
    };
    let encoded = serde_json::to_string(&reply)?;
    assert_eq!(encoded, r#"{"List":{"paused":false,"entries":[]}}"#);

    Ok(())
  }

  #[test]
  fn previews_accept_emitted_labels_and_reject_unknown_or_unbounded_input() {
    let valid = format!(r#"{{"id":1,"label":"{}… "}}"#, "é".repeat(95));
    let oversized = format!(r#"{{"id":1,"label":"{}"}}"#, "é".repeat(98));
    assert!(matches!(
      serde_json::from_str::<Preview>(&valid),
      Ok(_label)
    ));
    assert!(matches!(
      serde_json::from_str::<Preview>(&oversized),
      Err(_too_long)
    ));
    let unknown = r#"{"id":1,"label":"a","extra":true}"#;
    assert!(matches!(
      serde_json::from_str::<Preview>(unknown),
      Err(_extra_field)
    ));
  }
}
