mod chromium;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
  let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();

  match arguments.as_slice() {
    [mode, profile] => match (mode == chromium::POOL_FLAG).then_some(()) {
      Some(()) => {
        let Ok(diagnostics) = diagnostics();
        let Ok(pool_main) = chromium::pool_main(PathBuf::from(profile), diagnostics);
        pool_main
      }
      None => ExitCode::FAILURE,
    },
    _ => ExitCode::FAILURE,
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
