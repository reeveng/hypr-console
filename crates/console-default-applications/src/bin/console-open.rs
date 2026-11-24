//! Open a path or URL with the desktop's default application.

use std::os::unix::process::CommandExt;
use std::process::ExitCode;

use console_default_applications::opening;

fn main() -> ExitCode {
  let targets: Vec<_> = std::env::args_os().skip(1).collect();

  match targets.as_slice() {
    [target] => {
      let Ok(mut command) = opening::command(target);
      let fault = command.exec();

      eprintln!("console-open: {fault}");
    }
    _ => eprintln!("usage: console-open PATH-OR-URL"),
  }

  ExitCode::FAILURE
}
