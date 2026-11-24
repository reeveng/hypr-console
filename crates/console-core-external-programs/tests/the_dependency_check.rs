#![cfg(unix)]

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::OpenOptionsExt;
use std::process::Command;

type Failure = Box<dyn std::error::Error>;

#[test]
#[cfg_attr(
  dylint_lib = "explicit086_no_climbing_join",
  expect(
    explicit086_no_climbing_join,
    reason = "the fixed byte string is one non-UTF-8 filename component"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit040_no_torn_write",
  expect(
    explicit040_no_torn_write,
    reason = "the fixture intentionally creates a new empty file with executable permissions"
  )
)]
fn the_check_reports_all_results_without_starting_the_executables() -> Result<(), Failure> {
  let root = console_core_temporary_directories::fresh("dependency-check")?;
  let directory = root.join(OsString::from_vec(b"bin-\xff".to_vec()));
  std::fs::create_dir(&directory)?;
  let executable = directory.join("present-tool");
  let _fixture = std::fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .mode(0o755)
    .open(&executable)?;

  let answer = Command::new(env!("CARGO_BIN_EXE_console-dependencies"))
    .args(["executable", "absent-tool", "present-tool"])
    .env("PATH", &directory)
    .output()?;
  let report = String::from_utf8(answer.stdout)?;

  assert_eq!(answer.status.code(), Some(1));
  assert_eq!(
    report,
    concat!(
      "missing  executable absent-tool\npresent  executable present-tool\n",
      "\nOne selected dependency is missing.\n"
    )
  );

  let reported = Command::new(env!("CARGO_BIN_EXE_console-dependencies"))
    .args(["--report-only", "executable", "absent-tool", "present-tool"])
    .env("PATH", &directory)
    .output()?;
  let reported_text = String::from_utf8(reported.stdout)?;

  assert!(reported.status.success());
  assert_eq!(reported_text, report);

  std::fs::remove_dir_all(root)?;

  Ok(())
}
