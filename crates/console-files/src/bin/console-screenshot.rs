//! The full-screen shortcut used by the controller and keyboard.

use std::process::ExitCode;

fn main() -> ExitCode {
  match console_capture::screenshot(None) {
    Ok(path) => {
      println!("{}", path.display());
      ExitCode::SUCCESS
    }
    Err(fault) => {
      console_core_logging::error!("console-screenshot: {fault}");
      ExitCode::FAILURE
    }
  }
}
