//! Exercise panel requests and wait for their reported milestones.

use std::fs;
use std::io;
use std::os::unix::fs::FileTypeExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_places::Base;
use console_operating_system_linux::program_lifetime::alongside;
use console_panel::completion::{ENVIRONMENT, Progress};
use console_waiting::{Schedule, until_some_handed};
use rustix::process::Signal;

const POLL: Duration = Duration::from_millis(50);
const DEADLINE: Duration = Duration::from_secs(10);

pub struct Request<'a> {
  pub count: &'a str,
  pub save_to: &'a Path,
  pub program: &'a str,
  pub arguments: &'a [String],
}

pub fn run(request: Request<'_>) -> io::Result<()> {
  let count = request
    .count
    .parse::<u32>()
    .map_err(|fault| io::Error::new(io::ErrorKind::InvalidInput, fault))?;
  let Ok(runtime) = console_core_places::runtime_or_user();
  let scratch = console_core_temporary_directories::private_under(&runtime, "panel-completion")
    .map_err(io::Error::other)?;

  let result = cycle(
    count,
    &scratch,
    &runtime,
    request.program,
    request.arguments,
  )
  .and_then(|()| {
    let Ok(configured) = Base::State.configured();
    let state = configured
      .ok_or_else(|| io::Error::other("XDG_STATE_HOME is missing from the staged session"))?;
    fs::copy(state.join("console/waited.jsonl"), request.save_to).map(|_| ())
  });

  result.and(console_test_desktop::session::remove_tree(&scratch))
}

struct RequestFilename(String);

impl From<u32> for RequestFilename {
  fn from(request: u32) -> Self {
    Self(request.to_string())
  }
}

impl AsRef<Path> for RequestFilename {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

#[cfg_attr(
  dylint_lib = "explicit029_no_asking_per_item",
  expect(
    explicit029_no_asking_per_item,
    reason = "each benchmark sample measures a fresh panel request followed by a separate close"
  )
)]
fn cycle(
  count: u32,
  scratch: &Path,
  runtime: &Path,
  program: &str,
  arguments: &[String],
) -> io::Result<()> {
  let Ok(panels) = InternalProgram::Panels.name();
  let Ok(put_away) = InternalProgram::PutAway.name();
  let mut host = alongside(&mut Command::new(panels))?;
  let socket = runtime
    .join(console_core_places::APPLICATION)
    .join(console_panel::handoff::SOCKET);
  let Ok(schedule) = Schedule::asking_every(DEADLINE, POLL);
  let Ok(listening) = until_some_handed(schedule, &mut (), |()| {
    Ok::<_, Never>(
      socket
        .symlink_metadata()
        .is_ok_and(|file| file.file_type().is_socket())
        .then_some(()),
    )
  });
  listening
    .ok_or_else(|| io::Error::other(format!("panel host did not bind {}", socket.display())))?;

  let mut sending = Command::new(program);
  sending.args(arguments);
  let mut close = Command::new(put_away);

  for request in 0..count {
    let completion = scratch.join(RequestFilename::from(request));
    sending.env(ENVIRONMENT, &completion);
    let _request = alongside(&mut sending)?;

    wait(&completion, Progress::Accepted)?;
    wait(&completion, Progress::Visible)?;
    let closed = close.status()?;

    match ::core::primitive::bool::then_some(closed.success(), ()) {
      ::core::option::Option::Some(()) => wait(&completion, Progress::Gone)?,
      ::core::option::Option::None => {
        return Err(io::Error::other(format!("console-put-away: {closed}")));
      }
    }
  }

  host.interrupt_and_wait(Signal::TERM, Duration::from_secs(2))?;
  Ok(())
}

fn wait(completion: &Path, wanted: Progress) -> io::Result<()> {
  let Ok(word) = wanted.word();
  let Ok(schedule) = Schedule::asking_every(DEADLINE, POLL);
  let Ok(mut clock) = console_waiting::clock::LiveClock::started();
  let observed =
    console_waiting::until_some_handed_on_result(&mut clock, schedule, &mut (), |()| {
      let contents = match fs::read_to_string(completion) {
        Ok(contents) => contents,
        Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
          Some(()) => return Ok(None),
          None => return Err(fault),
        },
      };
      let mut progress = contents.lines().filter_map(|line| {
        let Ok(progress) = Progress::read(line);
        progress
      });
      let observed = progress
        .find(|mark| *mark == wanted || matches!(mark, Progress::Failed | Progress::Gone));

      match observed.map(|mark| (mark, (mark == wanted).then_some(()))) {
        Some((_, Some(()))) => Ok(Some(())),
        Some((mark, None)) => Err(io::Error::other(format!(
          "panel reported {mark:?} before {word}"
        ))),
        None => Ok(None),
      }
    })?;
  observed.ok_or_else(|| {
    io::Error::new(
      io::ErrorKind::TimedOut,
      format!("panel did not report {word}"),
    )
  })
}
