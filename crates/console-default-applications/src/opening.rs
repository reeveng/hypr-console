//! Open a file or URL and read or change the desktop's file associations.

use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use console_bus::connection::Bus;
use console_bus::messages::{Kind, Message, Value, Whom};
use console_core_external_programs::{Program, Unprinted, capture_output};
use console_core_never::Never;

pub fn command(target: &OsStr) -> Result<Command, Never> {
  let Ok(mut opening) = Program::XdgOpen.command();

  opening.arg(target);

  Ok(opening)
}

pub fn arguments(target: &str) -> Result<Vec<String>, Never> {
  Program::XdgOpen.arguments(&[target])
}

fn show_item_call(path: &Path) -> ::core::result::Result<Message, ::console_core_never::Never> {
  const FILE_MANAGER: &str = "org.freedesktop.FileManager1";
  let Ok(uri) = console_core_file_urls::url(path);
  let Ok(call) = Message::call(&Whom {
    to: FILE_MANAGER,
    at: "/org/freedesktop/FileManager1",
    on: FILE_MANAGER,
    calling: "ShowItems",
  });
  let Ok(call_2) = call.carrying(
    "ass",
    vec![
      Value::List(vec![Value::Word(uri)]),
      Value::Word(String::new()),
    ],
  );
  ::core::result::Result::Ok(call_2)
}

pub fn show_item(path: &Path, now: impl FnOnce() -> Instant) -> io::Result<()> {
  let ::core::result::Result::Ok(call) = show_item_call(path);
  let mut bus = Bus::session().map_err(io::Error::other)?;
  let deadline = now()
    .checked_add(Duration::from_secs(3))
    .ok_or_else(|| io::Error::other("invalid file manager deadline"))?;
  let reply = bus.call_before(&call, deadline).map_err(io::Error::other)?;

  match reply.kind {
    Kind::Answer => Ok(()),
    Kind::Call | Kind::ErrorReply | Kind::Signal => Err(io::Error::other(match reply.fault {
      Some(present) => present,
      None => "invalid file manager reply".to_owned(),
    })),
  }
}

pub fn default_for(mime: &str) -> Result<String, Unprinted> {
  let Ok(arguments) = Program::XdgMime.arguments(&["query", "default", mime]);

  capture_output(&arguments).map(|said| said.trim().to_owned())
}

pub struct Association<'a> {
  pub mime: &'a str,
  pub desktop: &'a str,
}

pub fn set_default(Association { mime, desktop }: Association<'_>) -> Result<(), Unprinted> {
  let Ok(arguments) = Program::XdgMime.arguments(&["default", desktop, mime]);

  capture_output(&arguments).map(|_| ())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reveal_sends_one_file_uri_to_the_desktop_file_manager() {
    let ::core::result::Result::Ok(request) =
      show_item_call(Path::new("/Downloads/first copy#1.pdf"));
    assert_eq!(
      request.destination.as_deref(),
      Some("org.freedesktop.FileManager1")
    );
    assert_eq!(request.member.as_deref(), Some("ShowItems"));
    assert_eq!(request.shape, "ass");
    assert_eq!(
      request.values,
      [
        Value::List(vec![Value::Word(
          "file:///Downloads/first%20copy%231.pdf".to_owned()
        )]),
        Value::Word(String::new()),
      ]
    );
  }
}
