//! Process and filesystem operations behind the Justfile's development recipes.

use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io::{self, ErrorKind};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus, Stdio};

use console_core_external_programs::Program;

mod explicit;
mod machine;

const NIGHTLY: &str = "+nightly-2026-05-28";
const SERVO: &str = "crates/console-browser-renderer-servo";

pub(super) fn run(arguments: &[String]) -> io::Result<()> {
  match arguments {
    [name, flags @ ..] => match name.as_str() {
      "explicit" | "literals" => explicit::summary(name),
      "explicit-rust" | "explicit-batch" | "explicit-repair" | "explicit-apis" => {
        explicit::migration(name, flags)
      }
      "explicit-fix"
      | "explicit-fix-all"
      | "explicit-fix-reviewed"
      | "explicit-fix-branches"
      | "explicit-wrap"
      | "explicit-index-bulk"
      | "explicit-av1-transform-bounds"
      | "explicit-shadow-av1" => explicit::fix(name, flags),
      "unsafe-rust" => unsafe_rust(flags),
      "kernel-image"
      | "kernel"
      | "kernel-input"
      | "kernel-browser"
      | "native-build"
      | "native-boot"
      | "native-check"
      | "compute-worker"
      | "compute-benchmark"
      | "video-presentation-benchmark" => machine::run(name, flags),
      "browser-pad-wasm" => browser_pad(),
      "browser" | "browser-servo-build" | "browser-servo" => browser(name, flags),
      "bench-ui" => bench_ui(flags),
      "words" => words(),
      "shot" => shot(),
      "capture" => capture_devices(),
      "lengths" => lengths(),
      "clean" => clean(),
      _unknown => Err(io::Error::other(format!("unknown Just recipe {name:?}"))),
    },
    [] => Err(io::Error::other("expected a Just recipe name")),
  }
}

fn command(program: Program) -> Command {
  let Ok(command) = program.command();

  command
}

fn cargo(arguments: &[&str]) -> Command {
  let mut cargo = command(Program::Cargo);
  cargo.args(arguments);

  cargo
}

fn status(command: &mut Command) -> io::Result<ExitStatus> {
  command
    .status()
    .map_err(|fault| io::Error::other(format!("could not run {command:?}: {fault}")))
}

fn checked(command: &mut Command) -> io::Result<()> {
  let exit = status(command)?;
  require_success(exit, command)
}

fn require_success(exit: ExitStatus, command: &Command) -> io::Result<()> {
  match exit.success().then_some(()) {
    Some(()) => Ok(()),
    None => Err(child_exited(format!("{command:?}"), exit)),
  }
}

#[derive(Debug)]
struct ChildExited {
  command: String,
  status: ExitStatus,
}

impl fmt::Display for ChildExited {
  fn fmt(&self, text: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(text, "{} exited with {}", self.command, self.status)
  }
}

impl std::error::Error for ChildExited {}

fn child_exited(command: String, status: ExitStatus) -> io::Error {
  io::Error::other(ChildExited { command, status })
}

pub(super) fn exit_code(fault: &io::Error) -> ExitCode {
  let code = fault
    .get_ref()
    .and_then(|fault| fault.downcast_ref::<ChildExited>())
    .and_then(|child| child.status.code())
    .and_then(|code| u8::try_from(code).ok());

  match code {
    Some(code) => ExitCode::from(code),
    None => ExitCode::FAILURE,
  }
}

fn output(command: &mut Command) -> io::Result<Vec<u8>> {
  let result = command.stderr(Stdio::inherit()).output()?;
  require_success(result.status, command)?;

  Ok(result.stdout)
}

fn env_path(key: &str, fallback: impl AsRef<Path>) -> PathBuf {
  match std::env::var_os(key).filter(|value| !value.is_empty()) {
    Some(value) => PathBuf::from(value),
    None => fallback.as_ref().to_path_buf(),
  }
}

fn target_dir() -> PathBuf {
  env_path("CARGO_TARGET_DIR", "target")
}

fn temporary_dir() -> PathBuf {
  let system = Path::new("/var/tmp");
  let user = std::env::var_os("USER")
    .and_then(|user| Path::new(&user).file_name().map(|name| system.join(name)));
  let fallback = user
    .filter(|path| path.is_dir())
    .unwrap_or_else(|| system.to_path_buf());

  env_path("TMPDIR", fallback)
}

fn install(source: &Path, target: &Path) -> io::Result<()> {
  fs::create_dir_all(parent(target))?;

  let bytes = fs::read(source)?;
  let mode = fs::Permissions::from_mode(0o644);
  console_core_atomic_writes::whole_with_permissions(target, &bytes, mode)
    .map_err(io::Error::other)
}

fn parent(path: &Path) -> &Path {
  path
    .parent()
    .filter(|parent| !parent.as_os_str().is_empty())
    .unwrap_or_else(|| Path::new("."))
}

fn unsafe_rust(arguments: &[String]) -> io::Result<()> {
  let directory = Path::new("target/unsafe-rust");
  fs::create_dir_all(directory)?;
  let executable = directory.join("check");
  let mut rustc = command(Program::Rustc);
  let sysroot = output(rustc.args([NIGHTLY, "--print", "sysroot"]))?;
  let sysroot = std::str::from_utf8(&sysroot).map_err(io::Error::other)?;
  let libraries = Path::new(sysroot.trim_end()).join("lib");
  let mut link = std::ffi::OsString::from("link-args=-Wl,-rpath,");
  link.push(libraries.as_os_str());

  checked(
    command(Program::Rustc)
      .args([NIGHTLY, "--edition=2024", "-O", "-L"])
      .arg(libraries)
      .arg("-C")
      .arg(link)
      .args(["tools/unsafe-rust.rs", "-o"])
      .arg(&executable),
  )?;
  checked(Command::new(executable).args(arguments))
}

fn browser_pad() -> io::Result<()> {
  let wasm = Path::new("target/browser-pad/pad.wasm");
  fs::create_dir_all(parent(wasm))?;
  checked(
    command(Program::Rustc)
      .args([
        "+1.98.0",
        "--edition=2024",
        "--crate-type=cdylib",
        "--target=wasm32-unknown-unknown",
        "-C",
        "opt-level=z",
        "-C",
        "lto",
        "-C",
        "panic=abort",
        "-C",
        "strip=symbols",
        "crates/console-browser-extension/wasm/pad.rs",
        "-o",
      ])
      .arg(wasm),
  )?;
  install(
    wasm,
    Path::new("crates/console-browser-extension/web/pad.wasm"),
  )
}

fn browser(name: &str, arguments: &[String]) -> io::Result<()> {
  let servo = env_path("CONSOLE_SERVO_TARGET_DIR", Path::new(SERVO).join("target"));
  let worker = servo.join("debug/console-browser-renderer-servo");
  match name {
    "browser" => {
      checked(&mut cargo(&[
        "build",
        "--quiet",
        "--locked",
        "-p",
        "console-application-browser",
        "-p",
        "console-browser-renderer-chromium",
      ]))?;
      checked(
        Command::new(target_dir().join("debug/console-application-browser"))
          .env("CONSOLE_BROWSER_SERVO_WORKER", worker)
          .args(arguments),
      )
    }
    "browser-servo-build" => checked(
      cargo(&["build", "--locked", "--manifest-path"])
        .arg(Path::new(SERVO).join("Cargo.toml"))
        .arg("--target-dir")
        .arg(servo),
    ),
    "browser-servo" => checked(
      cargo(&[
        "run",
        "--quiet",
        "--locked",
        "-p",
        "console-application-browser",
        "--",
        "--servo-worker",
      ])
      .arg(worker)
      .args(arguments),
    ),
    _unknown => Err(io::Error::other(format!("unknown browser recipe {name:?}"))),
  }
}

fn bench_ui(arguments: &[String]) -> io::Result<()> {
  checked(
    cargo(&[
      "bench",
      "--locked",
      "-p",
      "console-status-bar",
      "-p",
      "console-application-browser",
      "-p",
      "console-draw-painting",
      "-p",
      "console-applications",
      "-p",
      "console-application-files",
      "-p",
      "console-application-calculator",
      "--bench",
      "status_bar",
      "--bench",
      "browser_ui",
      "--bench",
      "picture_painting",
      "--bench",
      "launcher_search",
      "--bench",
      "folder_listing",
      "--bench",
      "calculator_keypad",
      "--",
    ])
    .args(arguments),
  )
}

fn words() -> io::Result<()> {
  let result = output(&mut cargo(&["x", "test", "vocabulary"]))?;
  let result = std::str::from_utf8(&result).map_err(io::Error::other)?;
  let log = fs::read(Path::new(result.trim_end()).join("console.log"))?;
  use std::io::Write;
  io::stdout().lock().write_all(&log)
}

fn shot() -> io::Result<()> {
  checked(&mut cargo(&[
    "x",
    "alone",
    "cargo",
    "run",
    "--quiet",
    "--bin",
    "console-desktop",
    "--",
    "shot",
    "desktop.png",
  ]))?;
  println!("desktop.png");

  Ok(())
}

fn from_device(remote_command: &str) -> io::Result<Vec<u8>> {
  let host = std::env::var_os("CONSOLE_HOST")
    .filter(|value| !value.is_empty())
    .ok_or_else(|| io::Error::other("set CONSOLE_HOST to the SSH destination for the device"))?;
  output(
    command(Program::Ssh)
      .arg("--")
      .arg(host)
      .arg(remote_command),
  )
}

fn capture_devices() -> io::Result<()> {
  let captured =
    from_device("cd /etc/console && cargo run --release --locked --quiet --bin capture-devices")?;
  save_fixture(
    "crates/console-input-gamepad/fixtures/devices.json",
    &captured,
  )
}

fn lengths() -> io::Result<()> {
  let captured = from_device("cat /home/*/.local/state/console/checked")?;
  let mut lines: Vec<_> = captured
    .split_inclusive(|byte| *byte == b'\n')
    .map(|line| line.strip_suffix(b"\n").unwrap_or(line))
    .collect();
  lines.sort_unstable();
  let mut sorted = Vec::with_capacity(captured.len());
  for line in lines {
    sorted.extend_from_slice(line);
    sorted.push(b'\n');
  }

  save_fixture("crates/console-test-stages/fixtures/lengths", &sorted)
}

fn save_fixture(path: &str, bytes: &[u8]) -> io::Result<()> {
  console_core_atomic_writes::whole(Path::new(path), bytes).map_err(io::Error::other)?;
  checked(command(Program::Git).args(["diff", "--stat", "--", path]))
}

fn clean() -> io::Result<()> {
  for directory in [".stage", "target"] {
    match fs::symlink_metadata(directory) {
      Ok(metadata) => match metadata.is_dir().then_some(()) {
        Some(()) => fs::remove_dir_all(directory)?,
        None => fs::remove_file(directory)?,
      },
      Err(fault) => match (fault.kind() == ErrorKind::NotFound).then_some(()) {
        Some(()) => {}
        None => return Err(fault),
      },
    }
  }

  Ok(())
}

fn require_file(path: &Path) -> io::Result<()> {
  match fs::metadata(path)?.is_file().then_some(()) {
    Some(()) => Ok(()),
    None => Err(io::Error::other(format!(
      "{} is not a file",
      path.display()
    ))),
  }
}

fn prefixed(prefix: &str, value: impl AsRef<OsStr>) -> std::ffi::OsString {
  let mut word = std::ffi::OsString::from(prefix);
  word.push(value);

  word
}
