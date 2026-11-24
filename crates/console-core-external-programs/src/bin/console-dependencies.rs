#[cfg(unix)]
mod dependencies;

#[cfg(unix)]
fn main() -> std::process::ExitCode {
  let Ok(exit) = dependencies::run();
  exit
}

#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
  eprintln!("console-dependencies requires a Unix host");
  std::process::ExitCode::FAILURE
}
