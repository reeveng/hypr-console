//! What a fault says, and how often it says it.
//!
//! Everything `console-say` is called from is a loop of some kind: a service
//! that restarts, a daemon that comes round every five minutes, an apply that
//! walks a list. The first few notifications tell someone something is wrong.
//! The two hundredth is a machine shouting over itself, and the way that ends
//! is with notifications turned off and the fault still there.
//!
//! So the journal gets everything and the screen gets a few, and that split is
//! what these assert. Run against the programs themselves, with a `notify-send`
//! and a `logger` of the test's own on the path, because what is worth knowing
//! is what the program does and not what libnotify does.
//!
//! These were written against two shell scripts and are unchanged in what they
//! ask. That is the whole reason for keeping them: the programs are new, and
//! what a person meets is meant not to be.

use std::error::Error;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use console_core_external_programs::Program;

const SAY: &str = env!("CARGO_BIN_EXE_console-say");
const REPORT_CRASH: &str = env!("CARGO_BIN_EXE_console-report-crash");
#[cfg(target_os = "linux")]
const UPLOAD: &str = env!("CARGO_BIN_EXE_console-upload-errors");

const LOUD: u32 = 5;

const AGAIN: (&str, &str) = ("wallpaper-choice", "The wallpaper was not changed");

type Failure = Box<dyn Error>;

struct Subscriber {
  here: PathBuf,
}

struct FixturePath<'a>(&'a Path);

impl AsRef<Path> for FixturePath<'_> {
  fn as_ref(&self) -> &Path {
    self.0
  }
}

impl Subscriber {
  fn new(named: &str) -> Result<Self, Failure> {
    let here = console_core_temporary_directories::fresh(&format!("legion-saying-{named}"))?;

    std::fs::create_dir_all(here.join("bin"))?;
    std::fs::create_dir_all(here.join("run"))?;

    let listening = Subscriber { here };
    let Ok(logger) = Program::Logger.name();

    listening.stub(("notify-send", "shown"))?;
    listening.stub((logger, "written"))?;

    Ok(listening)
  }

  fn stub(&self, (program, into): (&str, &str)) -> Result<(), Failure> {
    let at = self.program(program)?;
    let output = self.path(into)?;
    let script = format!("#!/bin/sh\necho \"$@\" >> {}\n", output.display());

    console_core_atomic_writes::whole_with_permissions(
      &at,
      script.as_bytes(),
      std::fs::Permissions::from_mode(0o755),
    )?;

    Ok(())
  }

  fn run(&self, program: &str, arguments: &[&str], result: Option<&str>) -> Result<(), Failure> {
    let mut command = self.command(program, arguments, result)?;
    let _status = command.status()?;

    Ok(())
  }

  fn command(
    &self,
    program: &str,
    arguments: &[&str],
    result: Option<&str>,
  ) -> Result<Command, Failure> {
    #[cfg_attr(
      dylint_lib = "explicit026_env_read_once",
      allow(
        explicit026_env_read_once,
        reason = "the program is run with the stubs in front of the path this test was given, so the path it was given is what is read"
      )
    )]
    let given = std::env::var("PATH")?;
    let path = format!("{}:{given}", self.here.join("bin").display());
    let mut running = Command::new(program);

    running
      .args(arguments)
      .env("PATH", path)
      .env("XDG_RUNTIME_DIR", self.here.join("run"))
      .env("XDG_STATE_HOME", self.here.join("state"))
      .env_remove("ORBIT_ERROR_DSN");

    match result {
      Some(reason) => {
        running.env("SERVICE_RESULT", reason);
      }
      None => {}
    }

    Ok(running)
  }

  fn say(&self, (kind, summary): (&str, &str)) -> Result<(), Failure> {
    self.run(SAY, &[kind, summary, "the body"], None)
  }

  fn path(&self, relative: &str) -> Result<PathBuf, Failure> {
    let path = Path::new(relative);

    match ::core::primitive::bool::then_some(
      !relative.is_empty()
        && !relative.contains('\0')
        && path
          .components()
          .all(|part| matches!(part, Component::Normal(_))),
      (),
    ) {
      ::core::option::Option::Some(()) => Ok(self.here.join(FixturePath(path))),
      ::core::option::Option::None => Err(Failure::from(format!(
        "invalid relative fixture path {relative:?}"
      ))),
    }
  }

  fn program(&self, name: &str) -> Result<PathBuf, Failure> {
    self.path(&format!("bin/{name}"))
  }

  fn read(&self, what: &str) -> Result<String, Failure> {
    let at = self.path(what)?;
    let contents = console_core_atomic_writes::text_or_empty(&at)?;

    Ok(contents)
  }

  fn counted(&self, what: &str) -> Result<u32, Failure> {
    let said = self.read(what)?;
    let Ok(lines) = console_core_number_conversion::fitted::<_, u32>(said.lines().count());

    Ok(lines)
  }
}

impl Drop for Subscriber {
  fn drop(&mut self) {
    let _cleanup_cannot_fail_the_notification_test = std::fs::remove_dir_all(&self.here);
  }
}

#[test]
fn a_failed_journal_command_reports_the_original_message_on_stderr() -> Result<(), Failure> {
  let listening = Subscriber::new("unwritten")?;
  let Ok(logger) = Program::Logger.name();
  let at = listening.program(logger)?;
  console_core_atomic_writes::whole_with_permissions(
    &at,
    b"#!/bin/sh\necho 'not delivered'\nexit 1\n",
    std::fs::Permissions::from_mode(0o755),
  )?;

  let mut result = listening.command(SAY, &["example", "the original fault", "details"], None)?;
  let output = result.output()?;
  let stderr = String::from_utf8(output.stderr)?;
  let shown = listening.counted("shown")?;

  assert!(output.status.success());
  assert!(stderr.contains("journal delivery failed: example: the original fault - details"));
  assert_eq!(shown, 1);

  Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn an_orbit_crash_report_stays_queued_until_accepted() -> Result<(), Failure> {
  let listening = Subscriber::new("orbit")?;
  let Ok(curl) = Program::Curl.name();
  let at = listening.program(curl)?;
  let receiver = br#"#!/bin/sh
cat > "$XDG_RUNTIME_DIR/auth"
for word
do
  case "$word" in
    @*.envelope) cat "${word#@}" > "$XDG_RUNTIME_DIR/received" || exit 1 ;;
    https://*) printf '%s' "$word" > "$XDG_RUNTIME_DIR/endpoint" ;;
  esac
done
printf '%s' "$TEST_HTTP_STATUS"
"#;
  console_core_atomic_writes::whole_with_permissions(
    &at,
    receiver,
    std::fs::Permissions::from_mode(0o755),
  )?;
  let dsn = "https://feedface@orbit.invalid/errors/1";
  let mut crash = listening.command(REPORT_CRASH, &["example.service"], Some("core-dump"))?;
  let without_reporting = crash.status()?;
  let runtime_reports = listening.here.join("run/console/error-reports");
  let state = listening.here.join("state");

  assert!(without_reporting.success());
  assert_missing(&runtime_reports)?;
  assert_missing(&state)?;

  console_core_atomic_writes::whole(
    &state,
    b"the reporting process cannot create a directory here",
  )?;
  let stopped = crash.env("ORBIT_ERROR_DSN", dsn).status()?;
  let queued = std::fs::metadata(runtime_reports)?;

  assert!(stopped.success());
  assert!(queued.is_dir());
  std::fs::remove_file(state)?;

  let mut upload = listening.command(UPLOAD, &[], None)?;
  upload.env("ORBIT_ERROR_DSN", dsn);
  let offline = upload.env("TEST_HTTP_STATUS", "000").status()?;
  assert!(offline.success());
  let report = listening.read("run/received")?;
  assert_crash_envelope(&report)?;
  let authorization = listening.read("run/auth")?;
  let endpoint = listening.read("run/endpoint")?;

  assert!(authorization.contains("sentry_key=feedface"));
  assert_eq!(endpoint, "https://orbit.invalid/errors/api/1/envelope/");

  std::fs::remove_file(listening.here.join("run/received"))?;
  let accepted = upload.env("TEST_HTTP_STATUS", "200").status()?;
  let resent = listening.read("run/received")?;

  assert!(accepted.success());
  assert_eq!(resent, report);

  std::fs::remove_file(listening.here.join("run/received"))?;
  let empty = upload.status()?;
  let remaining = listening.read("run/received")?;

  assert!(empty.success());
  assert!(remaining.is_empty());

  Ok(())
}

#[cfg(target_os = "linux")]
fn assert_missing(at: &Path) -> Result<(), Failure> {
  match std::fs::symlink_metadata(at) {
    Ok(_present) => Err(Failure::from(format!(
      "unexpected fixture {}",
      at.display()
    ))),
    Err(fault) => {
      match ::core::primitive::bool::then_some(fault.kind() == std::io::ErrorKind::NotFound, ()) {
        ::core::option::Option::Some(()) => Ok(()),
        ::core::option::Option::None => Err(Failure::from(fault)),
      }
    }
  }
}

#[cfg(target_os = "linux")]
fn assert_crash_envelope(report: &str) -> Result<(), Failure> {
  #[derive(serde::Deserialize)]
  #[serde(deny_unknown_fields)]
  struct Header {
    event_id: Box<str>,
  }

  #[derive(serde::Deserialize)]
  #[serde(deny_unknown_fields)]
  struct Item {
    #[serde(rename = "type")]
    kind: Box<str>,
  }

  #[derive(serde::Deserialize)]
  #[serde(deny_unknown_fields)]
  struct Message {
    formatted: Box<str>,
  }

  #[derive(serde::Deserialize)]
  #[serde(deny_unknown_fields)]
  struct Extra {
    component: Box<str>,
    severity: Box<str>,
    occurred_unix_seconds: Option<u64>,
  }

  #[derive(serde::Deserialize)]
  #[serde(deny_unknown_fields)]
  struct Event {
    event_id: Box<str>,
    level: Box<str>,
    logger: Box<str>,
    platform: Box<str>,
    timestamp: Option<u64>,
    message: Message,
    extra: Extra,
  }

  let mut lines = report.lines();
  let header_line = lines.next().ok_or("missing envelope")?;
  let item_line = lines.next().ok_or("missing item")?;
  let event_line = lines.next().ok_or("missing event")?;
  let header: Header = serde_json::from_str(header_line)?;
  let item: Item = serde_json::from_str(item_line)?;
  let event: Event = serde_json::from_str(event_line)?;

  assert_eq!(header.event_id, event.event_id);
  assert_eq!(item.kind.as_ref(), "event");
  assert_eq!(event.level.as_ref(), "fatal");
  assert_eq!(event.logger.as_ref(), "example.service");
  assert_eq!(event.platform.as_ref(), "native");
  assert!(event.message.formatted.contains("core-dump"));
  assert_eq!(event.extra.component, event.logger);
  assert_eq!(event.extra.severity, event.level);
  assert_eq!(event.extra.occurred_unix_seconds, event.timestamp);

  Ok(())
}

#[test]
fn a_fault_that_keeps_happening_is_shown_a_few_times_and_written_down_every_time()
-> Result<(), Failure> {
  let listening = Subscriber::new("again")?;
  let many = LOUD.saturating_add(3);

  for _ in 0..many {
    listening.say(AGAIN)?;
  }

  let shown = listening.counted("shown")?;
  let written = listening.counted("written")?;

  assert_eq!(shown, LOUD);
  assert_eq!(written, many);

  Ok(())
}

#[test]
fn the_last_one_shown_says_that_it_is_the_last() -> Result<(), Failure> {
  let listening = Subscriber::new("last")?;

  for _ in 0..LOUD {
    listening.say(AGAIN)?;
  }

  let shown = listening.read("shown")?;
  let last = shown.lines().next_back().ok_or("nothing was shown")?;

  assert!(
    last.contains("Not shown again"),
    "the last one shown said only: {last}"
  );

  Ok(())
}

#[test]
fn two_kinds_of_fault_are_counted_apart() -> Result<(), Failure> {
  let listening = Subscriber::new("kinds")?;

  for _ in 0..LOUD {
    listening.say(AGAIN)?;
  }

  listening.say(("compositor", "The compositor stopped answering"))?;

  let shown = listening.counted("shown")?;

  assert_eq!(shown, LOUD.saturating_add(1));

  Ok(())
}

#[test]
fn a_service_that_was_asked_to_stop_says_nothing() -> Result<(), Failure> {
  let listening = Subscriber::new("clean")?;

  listening.run(REPORT_CRASH, &["console-paper.service"], Some("success"))?;

  let shown = listening.counted("shown")?;
  let written = listening.counted("written")?;

  assert_eq!(shown, 0);
  assert_eq!(written, 0);

  Ok(())
}

#[test]
fn a_service_that_fell_over_says_which_one_it_was() -> Result<(), Failure> {
  let listening = Subscriber::new("crash")?;

  listening.run(REPORT_CRASH, &["console-paper.service"], Some("core-dump"))?;

  let shown = listening.read("shown")?;
  let written = listening.read("written")?;

  assert!(
    shown.contains("console-paper.service"),
    "it did not name itself: {shown}"
  );
  assert!(
    written.contains("core-dump"),
    "the journal was not told what happened: {written}"
  );

  Ok(())
}
