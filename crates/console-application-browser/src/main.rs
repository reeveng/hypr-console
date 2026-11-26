mod clipboard;
mod download_catalog;
mod download_refresh;
mod download_view;
mod downloads;
mod engine_start;
mod file_picker;
mod native;
mod presentation;
mod reader;
mod renderer;
mod session;
mod settings;
mod toolbar;

use std::process::ExitCode;

fn main() -> ExitCode {
  let arguments = std::env::args_os().skip(1).collect();
  let Ok(code) = native::run(arguments);
  code
}
