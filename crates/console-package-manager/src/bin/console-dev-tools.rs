//! Inspect and prepare the tools for a Console development checkout.

use std::ffi::OsString;
use std::process::ExitCode;

use console_core_never::Never;
use console_package_manager::tools::{Operation, command};

const USAGE: &str = "\
Usage: console-dev-tools [status | install | update [--apply] | run -- COMMAND...]\n\
  status         Show the checkout's requested and installed tool versions\n\
  install        Install the versions recorded in mise.lock\n\
  update         Preview newer versions allowed by mise.toml\n\
  update --apply Record newer versions and checksums in mise.lock\n\
  run -- COMMAND Run with the checkout's installed tool versions on PATH";

enum Request<'a> {
  Help,
  Unsupported,
  Tools(Operation<'a>),
}

fn request(arguments: &[OsString]) -> Result<Request<'_>, Never> {
  Ok(match arguments.split_first() {
    None => Request::Tools(Operation::Status),
    Some((action, rest)) => match (action.to_str(), rest) {
      (Some("help" | "--help" | "-h"), []) => Request::Help,
      (Some("status"), []) => Request::Tools(Operation::Status),
      (Some("install"), []) => Request::Tools(Operation::Install),
      (Some("update"), []) => Request::Tools(Operation::PreviewUpdate),
      (Some("update"), [flag]) => match flag.to_str() {
        Some("--apply") => Request::Tools(Operation::UpdateLock),
        Some(_) | None => Request::Unsupported,
      },
      (Some("run"), [separator, command @ ..]) => match (separator.to_str(), command) {
        (Some("--"), [_, ..]) => Request::Tools(Operation::Run(command)),
        (Some(_) | None, _) => Request::Unsupported,
      },
      (Some(_) | None, _) => Request::Unsupported,
    },
  })
}

fn run(operation: Operation<'_>) -> Result<ExitCode, Box<dyn std::error::Error>> {
  let checkout = console_repository::root()?;
  let mut mise = command(&checkout, operation)?;
  let status = mise.status()?;

  Ok(match status.success().then_some(()) {
    Some(()) => ExitCode::SUCCESS,
    None => ExitCode::FAILURE,
  })
}

fn main() -> ExitCode {
  let arguments: Vec<_> = std::env::args_os().skip(1).collect();
  let Ok(requested) = request(&arguments);

  match requested {
    Request::Help => {
      println!("{USAGE}");
      ExitCode::SUCCESS
    }
    Request::Unsupported => {
      eprintln!("{USAGE}");
      ExitCode::from(2)
    }
    Request::Tools(operation) => match run(operation) {
      Ok(exit) => exit,
      Err(fault) => {
        eprintln!("console-dev-tools: {fault}");
        ExitCode::FAILURE
      }
    },
  }
}
