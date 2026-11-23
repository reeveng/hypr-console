mod chromium;
mod native;
mod reader;
mod renderer;
mod settings;
mod toolbar;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
  let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();

  match arguments.first() {
    Some(mode) => match (mode == chromium::WORKER).then_some(()) {
      Some(()) => match arguments.as_slice() {
        [_, profile] => {
          let Ok(diagnostics) = diagnostics();
          let Ok(code) = chromium::worker_main(PathBuf::from(profile), diagnostics);

          code
        }
        _ => ExitCode::FAILURE,
      },
      None => {
        let Ok(code) = native::run(arguments);

        code
      }
    },
    None => {
      let Ok(code) = native::run(arguments);

      code
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  expect(
    explicit026_env_read_once,
    reason = "the browser check runner sets CONSOLE_BROWSER_CHECK to retain Chromium stderr only for its diagnostic subprocess"
  )
)]
fn diagnostics() -> Result<chromium::Diagnostics, console_core_never::Never> {
  Ok(match std::env::var_os("CONSOLE_BROWSER_CHECK") {
    Some(value) => match (value == "1").then_some(()) {
      Some(()) => chromium::Diagnostics::Show,
      None => chromium::Diagnostics::Disabled,
    },
    None => chromium::Diagnostics::Disabled,
  })
}
