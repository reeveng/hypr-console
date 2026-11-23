//! Deliver queued diagnostics to the configured Orbit space.

#[cfg(target_os = "linux")]
use std::{
  ffi::OsString,
  io::{self, Write},
  path::Path,
  process::Stdio,
};

#[cfg(target_os = "linux")]
use console_core_external_programs::Program;
#[cfg(target_os = "linux")]
use console_core_logging::orbit::Destination;

fn main() {
  #[cfg(target_os = "linux")]
  match upload() {
    Ok(()) => {}
    Err(fault) => {
      match ::core::primitive::bool::then_some(fault.kind() == io::ErrorKind::NotConnected, ()) {
        ::core::option::Option::Some(()) => return,
        ::core::option::Option::None => {
          console_core_logging::warn!("Orbit reports are still queued: {fault}")
        }
      }
    }
  }
}

#[cfg(target_os = "linux")]
fn upload() -> io::Result<()> {
  let configured = Destination::configured()?;
  let destination = match configured {
    Some(destination) => destination,
    None => return Ok(()),
  };
  let Ok(queued) = destination.outbox();

  match queued {
    Some(outbox) => outbox.send_pending(|report| send(&destination, report)),
    None => Ok(()),
  }
}

#[cfg(target_os = "linux")]
fn send(destination: &Destination, report: &Path) -> io::Result<()> {
  let mut file = OsString::from("@");
  file.push(report);
  let Ok(endpoint) = destination.endpoint();
  let Ok(authorization) = destination.authorization();
  let Ok(mut command) = Program::Curl.command();
  let mut running = command
    .args([
      "--disable",
      "--silent",
      "--proto",
      "=https",
      "--max-time",
      "8",
      "--header",
      "Content-Type: application/x-sentry-envelope",
      "--header",
      "@-",
      "--output",
      "/dev/null",
      "--write-out",
      "%{http_code}",
      "--data-binary",
    ])
    .arg(file)
    .args(["--", endpoint])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .spawn()?;
  let written = match running.stdin.take() {
    Some(mut input) => writeln!(input, "{authorization}"),
    None => Err(io::Error::new(
      io::ErrorKind::BrokenPipe,
      "curl did not open its input",
    )),
  };
  let response = running.wait_with_output()?;
  let status = match std::str::from_utf8(&response.stdout) {
    Ok(code) => match code.parse::<u16>() {
      Ok(status) => Some(status),
      Err(_not_status) => None,
    },
    Err(_invalid_status_bytes) => None,
  };

  match (status, response.status.success()) {
    (Some(200..=299), true) => written,
    (None | Some(0), _) => Err(io::Error::from(io::ErrorKind::NotConnected)),
    (Some(code), _) => Err(io::Error::other(format!(
      "Orbit upload returned HTTP {code}"
    ))),
  }
}
