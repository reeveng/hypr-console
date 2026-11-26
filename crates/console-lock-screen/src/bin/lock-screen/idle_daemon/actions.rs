use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};

use console_compositor::{DispatchResult, DisplayPower};
use console_concurrency::Latest;
use console_core_external_programs::Program;
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_number_conversion::index;
use console_lock_screen::idle::policy::Display;
use console_operating_system_linux::program_lifetime::{BoundToParent, alongside};
use rustix::event::{PollFd, PollFlags, Timespec, poll};

pub struct Actions {
  display: Latest,
  wanted: SyncSender<Display>,
  lock: Latest,
  suspend: Latest,
}

impl Actions {
  pub fn start() -> io::Result<Self> {
    const PENDING_DISPLAY_CHANGES: u32 = 8;
    let Ok(capacity) = index(PENDING_DISPLAY_CHANGES);
    let (wanted, pending) = sync_channel::<Display>(capacity);
    let display = Latest::start("idle-display".to_owned(), move || {
      match pending.try_iter().last() {
        Some(next) => report("changing the idle display state", show(next)),
        None => Ok(()),
      }
    })?;
    let lock = Latest::start("idle-lock".to_owned(), || {
      let Ok(mut command) = Program::Systemctl.command();
      command.args([
        "--user",
        "--no-block",
        "start",
        console_lock_screen::LOCK_SERVICE,
      ]);
      report("starting the session lock", run(command))
    })?;
    let suspend = Latest::start("idle-suspend".to_owned(), || {
      let Ok(mut command) = Program::Systemctl.command();
      command.args([
        "--check-inhibitors=yes",
        "--no-ask-password",
        "--no-block",
        "suspend",
      ]);
      report("requesting automatic suspend", run(command))
    })?;

    Ok(Self {
      display,
      wanted,
      lock,
      suspend,
    })
  }

  pub fn display(&self, next: Display) -> io::Result<()> {
    match self.wanted.try_send(next) {
      Ok(()) => schedule(&self.display),
      Err(TrySendError::Full(_waiting)) => {
        Err(io::Error::other("the idle display queue is full"))
      }
      Err(TrySendError::Disconnected(_waiting)) => {
        Err(io::Error::other("idle display worker stopped"))
      }
    }
  }

  pub fn lock(&self) -> io::Result<()> {
    schedule(&self.lock)
  }

  pub fn suspend(&self) -> io::Result<()> {
    schedule(&self.suspend)
  }

  pub fn close(self) -> Result<(), Never> {
    for job in [self.display, self.lock, self.suspend] {
      match job.close() {
        Ok(()) => {}
        Err(_worker_panicked) => {
          console_core_logging::warn!("An idle action worker stopped unexpectedly")
        }
      }
    }

    Ok(())
  }
}

fn schedule(worker: &Latest) -> io::Result<()> {
  worker
    .schedule()
    .map_err(|_stopped| io::Error::other("idle action worker stopped"))
}

fn report(action: &str, result: io::Result<()>) -> Result<(), Never> {
  match result {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("{action}: {fault}"),
  }

  Ok(())
}

fn show(next: Display) -> io::Result<()> {
  let treatment = match next {
    Display::On => "undim",
    Display::Dimmed => {
      display_power(DisplayPower::On)?;
      "dim"
    }
    Display::Off => {
      display_power(DisplayPower::Off)?;
      console_core_logging::info!("Idle display powered off");
      return Ok(());
    }
  };
  let Ok(mut command) = InternalProgram::Brightness.command();
  command.arg(treatment);
  run(command)?;
  console_core_logging::info!("Idle display brightness action completed: {treatment}");
  Ok(())
}

fn display_power(power: DisplayPower) -> io::Result<()> {
  let Ok(outcome) = power.set();

  match outcome {
    DispatchResult::Success => Ok(()),
    DispatchResult::Failure(fault) => Err(io::Error::other(fault)),
  }
}

fn run(mut command: Command) -> io::Result<()> {
  let output = command.output()?;
  let status = output.status;

  match status.success().then_some(()) {
    Some(()) => Ok(()),
    None => {
      let detail = match std::str::from_utf8(&output.stderr) {
        Ok(text) => text.trim(),
        Err(_not_text) => "process diagnostics were not UTF-8",
      };

      Err(io::Error::other(format!("{status}: {detail}")))
    }
  }
}

pub struct SleepDelay {
  _child: BoundToParent,
  _input: std::process::ChildStdin,
  _output: std::process::ChildStdout,
}

impl SleepDelay {
  pub fn hold() -> io::Result<Self> {
    const WAIT: Timespec = Timespec {
      tv_sec: 2,
      tv_nsec: 0,
    };
    let Ok(mut command) = Program::SystemdInhibit.command();
    let Ok(cat) = Program::Cat.name();
    command
      .args([
        "--what=sleep",
        "--mode=delay",
        "--who=Console",
        "--why=Waiting for the session lock",
        cat,
      ])
      .stdin(Stdio::piped())
      .stdout(Stdio::piped());
    let mut child = alongside(&mut command)?;
    let Ok(input_pipe) = child.take_stdin();
    let Ok(output_pipe) = child.take_stdout();
    let mut input =
      input_pipe.ok_or_else(|| io::Error::other("sleep inhibitor has no input pipe"))?;
    let mut output =
      output_pipe.ok_or_else(|| io::Error::other("sleep inhibitor has no output pipe"))?;

    input.write_all(b"1")?;
    let mut pending = [PollFd::new(&output, PollFlags::IN)];

    match poll(&mut pending, Some(&WAIT)) {
      Ok(0) => {
        return Err(io::Error::new(
          io::ErrorKind::TimedOut,
          "acquiring the sleep inhibitor",
        ));
      }
      Ok(_ready) => {}
      Err(fault) => return Err(io::Error::from(fault)),
    }

    let mut reply = [0];
    #[cfg_attr(
      dylint_lib = "explicit057_no_unbounded_wait",
      expect(
        explicit057_no_unbounded_wait,
        reason = "the preceding bounded poll ensures the private pipe has a byte or has closed, and this is its only reader"
      )
    )]
    output.read_exact(&mut reply)?;

    match reply {
      [b'1'] => Ok(Self {
        _child: child,
        _input: input,
        _output: output,
      }),
      [_invalid] => Err(io::Error::other(
        "sleep inhibitor returned an invalid acknowledgment",
      )),
    }
  }
}
