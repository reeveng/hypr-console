//! Which hearing this device should be using, measured on this device.
//!
//!     voice-compare --record     say the sixteen clips, once
//!     voice-compare --models     fetch the models
//!     voice-compare --build      build the second engine, which is slow
//!     voice-compare --fetch      both of those
//!     voice-compare              read every clip with every model
//!  This runs ON the handheld, as the person whose session it is -- not as root
//! over ssh. Recording needs the microphone, and the microphone belongs to a
//! PipeWire that belongs to a session.  What is here is everything that touches
//! this machine, and it is the three effects
//! `console_input_dictation::comparing` decided but could not name: engines
//! that are binaries at paths rather than programs on the machine, a run that
//! has to be timed rather than only answered, and a recording that one press
//! begins and the next one ends.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus, Stdio};
use std::time::Instant;

use console_audio_streams::Recording;
use console_core_atomic_writes::{Stored, read};
use console_core_external_programs::Program as ExternalProgram;
use console_core_never::Never;
use console_input_dictation::comparing::{
  COMMAND, Candidate, CommandPath, Compare, CompareEffect, CompareEvent, Comparison, Found, Task,
  Timing, Where,
};
use console_input_dictation::{RECORDING_FORMAT, RECORDING_STOP_TIMEOUT};

use console_operating_system_linux::program_runtime::{Clock, Interpreter};
use console_program_contract::Event;

const NOTHING_TIMED: u128 = 0;

const TRIES: [u8; 3] = [1, 2, 3];

const NAMED: &str = "/proc/sys/kernel/hostname";

struct Machine {
  recording: Option<Clip>,
}

struct Clip {
  child: Recording,
  path: PathBuf,
}

#[derive(Debug)]
enum RecordingError {
  Active(PathBuf),
  Inactive,
  Start(PathBuf, io::Error),
  Stop(PathBuf, io::Error),
  ExitedEarly(PathBuf, ExitStatus),
  Exited(PathBuf, ExitStatus),
}

impl fmt::Display for RecordingError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Active(at) => write!(to, "already recording {}", at.display()),
      Self::Inactive => to.write_str("there is no recording to stop"),
      Self::Start(at, error) => write!(to, "could not start recording {}: {error}", at.display()),
      Self::Stop(at, error) => write!(to, "could not finish recording {}: {error}", at.display()),
      Self::ExitedEarly(at, status) => write!(
        to,
        "the recorder for {} exited before it was asked to stop ({status})",
        at.display()
      ),
      Self::Exited(at, status) => {
        write!(to, "the recorder for {} exited with {status}", at.display())
      }
    }
  }
}

impl std::error::Error for RecordingError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Self::Start(_, error) | Self::Stop(_, error) => Some(error),
      Self::Active(_) | Self::Inactive | Self::ExitedEarly(_, _) | Self::Exited(_, _) => None,
    }
  }
}

impl Clip {
  fn start(path: &Path) -> Result<Self, RecordingError> {
    let child = Recording::start(RECORDING_FORMAT, path)
      .map_err(|error| RecordingError::Start(path.to_path_buf(), error))?;

    Ok(Self {
      child,
      path: path.to_path_buf(),
    })
  }

  fn stop(self) -> Result<(), RecordingError> {
    let Self { mut child, path } = self;
    let already_exited = child
      .try_wait()
      .map_err(|error| RecordingError::Stop(path.clone(), error))?;

    match already_exited {
      Some(status) => return Err(RecordingError::ExitedEarly(path, status)),
      None => {}
    }

    let status = child
      .finish(RECORDING_STOP_TIMEOUT)
      .map_err(|error| RecordingError::Stop(path.clone(), error))?;

    match ::core::primitive::bool::then_some(status.success(), ()) {
      ::core::option::Option::Some(()) => Ok(()),
      ::core::option::Option::None => Err(RecordingError::Exited(path, status)),
    }
  }
}

impl Interpreter for Machine {
  type Event = CompareEvent;
  type Effect = CompareEffect;
  type Error = RecordingError;

  fn interpret(
    &mut self,
    acts: &CompareEffect,
    _clock: &Clock,
  ) -> Result<Vec<Event<CompareEvent>>, RecordingError> {
    let event = match acts {
      CompareEffect::Look(every) => {
        let seen = every
          .iter()
          .map(|at| {
            let Ok(found) = probe(at.as_ref());

            Candidate {
              at: at.clone(),
              is: found,
            }
          })
          .collect();

        CompareEvent::Looked(seen)
      }

      CompareEffect::Ran(arguments) => {
        let Ok(said) = run_stderr(arguments);

        CompareEvent::Output(said)
      }

      CompareEffect::Timed(arguments) => {
        let Ok(took) = timed(arguments);

        CompareEvent::Timed(took)
      }

      CompareEffect::StartRecording(into) => {
        match &self.recording {
          Some(recording) => return Err(RecordingError::Active(recording.path.clone())),
          None => {}
        }

        let started = Clip::start(into)?;
        self.recording = Some(started);

        CompareEvent::RecordingStarted
      }

      CompareEffect::StopRecording => {
        let recording = self.recording.take().ok_or(RecordingError::Inactive)?;
        recording.stop()?;

        CompareEvent::RecordingStopped
      }
    };

    Ok(vec![Event::Custom(event)])
  }

  fn finish(&mut self) -> Result<(), RecordingError> {
    match self.recording.take() {
      Some(recording) => recording.stop(),
      None => Ok(()),
    }
  }
}

fn probe(at: &std::path::Path) -> Result<Found, Never> {
  use std::os::unix::fs::PermissionsExt;

  Ok(match std::fs::metadata(at) {
    Err(_not_there) => Found::Absent,
    Ok(what) => match (what.is_file(), what.permissions().mode() & 0o111) {
      (true, 0) | (false, _) => Found::There,
      (true, _) => Found::Runnable,
    },
  })
}

fn starting(arguments: &[String]) -> Result<Option<Command>, Never> {
  let first = match arguments.first() {
    Some(first) => first,
    None => return Ok(None),
  };

  let mut starting = Command::new(first);

  starting.args(arguments.iter().skip(1));

  Ok(Some(starting))
}

fn run_stderr(arguments: &[String]) -> Result<String, Never> {
  let Ok(starting) = starting(arguments);

  Ok(match starting {
    None => String::new(),
    Some(mut starting_2) => match starting_2.stderr(Stdio::piped()).output() {
      Err(_would_not_start) => String::new(),
      Ok(done) => {
        let Ok(stdout) = output_text(done.stdout, "stdout");
        let Ok(stderr) = output_text(done.stderr, "stderr");

        format!("{stdout}{stderr}")
      }
    },
  })
}

fn timed(arguments: &[String]) -> Result<Timing, Never> {
  let mut first = None;
  let mut best = None;
  let mut heard = String::new();

  for _ in TRIES {
    let began = Instant::now();

    let Ok(starting) = starting(arguments);

    heard = match starting {
      None => String::new(),
      Some(mut starting_2) => match starting_2.stderr(Stdio::null()).output() {
        Err(_would_not_start) => String::new(),
        Ok(done) => {
          let Ok(stdout) = output_text(done.stdout, "stdout");

          stdout
        }
      },
    };

    let took = began.elapsed().as_millis();

    first = first.or(Some(took));
    best = Some(best.map_or(took, |best_2: u128| best_2.min(took)));
  }

  let first_3 = match first {
    Some(first_2) => first_2,
    None => NOTHING_TIMED,
  };

  let best_4 = match best {
    Some(best_3) => best_3,
    None => NOTHING_TIMED,
  };

  Ok(Timing {
    first: first_3,
    best: best_4,
    said: heard,
  })
}

fn output_text(bytes: Vec<u8>, stream: &str) -> Result<String, Never> {
  Ok(match String::from_utf8(bytes) {
    Ok(text) => text,
    Err(fault) => {
      eprintln!("voice-compare: the engine's {stream} was not UTF-8: {fault}");

      String::new()
    }
  })
}

fn here() -> Result<String, Never> {
  let Ok(held) = read(std::path::Path::new(NAMED));

  Ok(match held {
    Stored::Text(said) => said.trim().to_string(),
    Stored::Absent => String::new(),
    Stored::Failed(fault) => {
      eprintln!("voice-compare: {NAMED}: {fault}");

      String::new()
    }
  })
}

fn stamped() -> Result<String, Never> {
  let Ok(asking) = ExternalProgram::Date.arguments(&["+%Y-%m-%d-%H%M"]);

  Ok(
    match console_core_external_programs::capture_output(&asking) {
      Ok(said) => said.trim().to_string(),
      Err(_unprinted) => String::new(),
    },
  )
}

fn main() -> ExitCode {
  let Ok(kept) = console_input_dictation::models_path();

  let kept_3 = match kept {
    Some(kept_2) => kept_2,
    None => {
      eprintln!("voice-compare: nothing says where the recordings are kept");

      return ExitCode::FAILURE;
    }
  };

  let words: Vec<String> = std::env::args().skip(1).collect();

  let job = match console_core_arguments::read(&COMMAND, &words).and_then(|line| Task::of(&line))
  {
    Ok(job) => job,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };
  let Ok(host) = here();
  let Ok(stamp) = stamped();
  let directory = match CommandPath::new(kept_3) {
    Ok(directory) => directory,
    Err(fault) => {
      eprintln!("voice-compare: {fault}");

      return ExitCode::FAILURE;
    }
  };
  let comparison = Comparison {
    at: Where {
      kept: directory,
      host,
      stamp,
    },
    job,
  };
  let mut machine = Machine { recording: None };
  let Ok(code) = console_operating_system_linux::program_runtime::run::<Compare, Machine>(
    COMMAND.name,
    &comparison,
    &mut machine,
  );

  code
}
