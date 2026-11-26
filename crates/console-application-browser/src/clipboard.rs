//! Browser chrome exchanges text with the session's clipboard owner.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::net::Shutdown;
use std::process::Stdio;
use std::time::Duration;

use console_core_internal_programs::InternalProgram;
use console_operating_system_linux::unix_stream::{Within, connect};
use serde_json::{Value, json};

const MAX_REPLY_BYTES: u32 = 300 * 1024;
const DEADLINE: Duration = Duration::from_millis(250);

enum ClipboardReply {
  Copied,
  Unavailable,
  Text(String),
}

fn decode_reply(bytes: &[u8]) -> io::Result<ClipboardReply> {
  let leading = bytes
    .iter()
    .copied()
    .find(|byte| !byte.is_ascii_whitespace());

  match leading {
    Some(b'"') => {
      let status: String = serde_json::from_slice(bytes)?;

      match status.as_str() {
        "Done" => Ok(ClipboardReply::Copied),
        "Unavailable" => Ok(ClipboardReply::Unavailable),
        _ => Err(io::Error::other("unknown clipboard reply status")),
      }
    }
    Some(b'{') => {
      let mut fields: BTreeMap<String, String> = serde_json::from_slice(bytes)?;
      let text = fields.remove("Text");

      match (text, fields.is_empty().then_some(())) {
        (Some(value), Some(())) => Ok(ClipboardReply::Text(value)),
        _ => Err(io::Error::other("invalid clipboard text reply")),
      }
    }
    Some(_) | None => Err(io::Error::other("invalid clipboard reply")),
  }
}

fn exchange(request: Value) -> io::Result<ClipboardReply> {
  let Ok(runtime) = console_core_places::runtime();
  let directory = runtime.ok_or_else(|| io::Error::other("no clipboard runtime directory"))?;
  let path = directory.join("console-clipboard/history.sock");
  let encoded = serde_json::to_vec(&request)?;
  let connection = connect(&path)?;
  let mut timed = Within::new(&connection, DEADLINE)?;
  timed.write_all(&encoded)?;
  connection.shutdown(Shutdown::Write)?;
  let reply = console_core_bounded_reads::bytes(timed, MAX_REPLY_BYTES)?;
  decode_reply(&reply)
}

pub(crate) fn copy(text: &str) -> io::Result<()> {
  let reply = exchange(json!({ "CopyText": text }))?;

  match reply {
    ClipboardReply::Copied => Ok(()),
    ClipboardReply::Unavailable | ClipboardReply::Text(_) => {
      Err(io::Error::other("the clipboard refused the copy"))
    }
  }
}

pub(crate) fn paste() -> io::Result<Option<String>> {
  match exchange(json!("ReadText")) {
    Ok(ClipboardReply::Text(text)) => return Ok(Some(text)),
    Ok(ClipboardReply::Copied | ClipboardReply::Unavailable) => return Ok(None),
    Err(fault) => console_core_logging::debug!(
      "Clipboard socket unavailable. Reading through the command: {}",
      fault.kind()
    ),
  }

  let Ok(mut command) = InternalProgram::Clipboard.command();
  let output = command
    .arg("--read-text")
    .stdin(Stdio::null())
    .stderr(Stdio::null())
    .output()?;

  match { ::core::primitive::bool::then_some(output.status.success(), ()) } {
    ::core::option::Option::Some(()) => {
      return String::from_utf8(output.stdout)
        .map(Some)
        .map_err(io::Error::other);
    }
    ::core::option::Option::None => {}
  }

  Ok(None)
}
