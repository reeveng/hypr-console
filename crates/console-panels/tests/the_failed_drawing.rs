//! The real host must not acknowledge a frame when no compositor can draw it.

use std::error::Error;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::time::Duration;

use console_operating_system_linux::program_lifetime::alongside;
use console_panel::handoff::{self, Request};
use console_waiting::clock::LiveClock;
use console_waiting::{Schedule, until_some_handed_on_result};

#[test]
fn an_accepted_request_that_cannot_draw_reports_failure_instead_of_drawn()
-> Result<(), Box<dyn Error>> {
  let here = console_core_temporary_directories::fresh("panel-cannot-draw")?;
  let runtime = here.join("runtime");

  std::fs::create_dir_all(runtime.join("console"))?;

  let mut starting = Command::new(env!("CARGO_BIN_EXE_console-panels"));
  starting
    .env("XDG_RUNTIME_DIR", &runtime)
    .env("HOME", &here)
    .env("XDG_STATE_HOME", here.join("state"))
    .env("WAYLAND_DISPLAY", here.join("missing-compositor"));

  let _host = alongside(&mut starting)?;
  let mut socket = runtime.join("console").join(handoff::SOCKET);
  let Ok(patience) = Schedule::of(Duration::from_secs(5));
  let Ok(mut clock) = LiveClock::started();
  let connected =
    until_some_handed_on_result(
      &mut clock,
      patience,
      &mut socket,
      |at| match UnixStream::connect(at) {
        Ok(stream) => Ok(Some(stream)),
        Err(fault) => match bool::then_some(
          [io::ErrorKind::NotFound, io::ErrorKind::ConnectionRefused].contains(&fault.kind()),
          (),
        ) {
          Some(()) => Ok(None),
          None => Err(fault),
        },
      },
    )?;

  let mut stream = connected.ok_or("the panel listener did not become ready")?;
  stream.set_read_timeout(Some(Duration::from_secs(5)))?;

  let request = Request {
    who: "calculator".to_string(),
    from: "test".to_string(),
    pressed: None,
    exec: Duration::ZERO,
    tells: None,
    arguments: Vec::new(),
  };
  let Ok(said) = handoff::serialize(&request);

  writeln!(stream, "{said}")?;

  let mut reading = BufReader::new(stream);
  let mut accepted = String::new();
  let mut finished = String::new();

  reading.read_line(&mut accepted)?;
  reading.read_line(&mut finished)?;

  assert_eq!(accepted.trim(), handoff::ACCEPTED);
  assert_eq!(finished.trim(), handoff::FAILED);

  Ok(())
}
