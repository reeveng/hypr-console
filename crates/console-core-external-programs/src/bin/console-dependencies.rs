#[cfg(unix)]
fn main() -> std::process::ExitCode {
  use console_core_external_programs::dependencies::{Invocation, run};
  use std::io;
  use std::process::ExitCode;

  let arguments: Vec<_> = std::env::args_os().skip(1).collect();

  match run(
    Invocation::Standalone,
    &arguments,
    &mut io::stdout().lock(),
    &mut io::stderr().lock(),
  ) {
    Ok(exit) => exit,
    Err(fault) => {
      eprintln!("console-dependencies: {fault}");
      ExitCode::FAILURE
    }
  }
}

#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
  eprintln!("console-dependencies requires a Unix host");
  std::process::ExitCode::FAILURE
}
