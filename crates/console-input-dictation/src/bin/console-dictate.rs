//! Speak, and have it typed.
//!
//! One press starts listening and the next one writes down what was said, so
//! the same button is the whole of it: there is nothing to hold and nothing to
//! aim at, which is what a button on the back of a device has to be.
//!
//! **Both presses say what they cost.** The first is the machine getting a
//! microphone running, which is a wait someone stands through wondering
//! whether the button did anything; the second is the whole of the model's
//! work, which is the longest thing this desktop asks of the processor with a
//! person waiting on it. Neither number can be had by standing at a terminal
//! with a stopwatch -- the press is on the back of the device -- so both are
//! written down where they happen, and the second one names its stretches, so a
//! sentence that took a while is answered by which of the model, the recording
//! or the typing took it.
//!
//! The recording is what says which press this is. A file in the runtime
//! directory holds the microphone's own process, and a press that finds it
//! stops. Nothing is remembered anywhere else, so a session that ends in the
//! middle of a sentence leaves a desktop that is not listening rather than one
//! that thinks it still is.

use std::ffi::OsString;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use console_core_arguments::{Presence, Reason, ValidationError};
use console_core_atomic_writes::Stored;
use console_core_external_programs::Program;
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_input_dictation::{
  BUILD, COMMAND, FETCH, Note, VoiceActivity, cloning, compiling, configuring, detect_speech,
  fetching, languages, making, model, recording, recording_path, serialize_note, taking, tidy,
  told_by, typing, whisper, whisper_arguments, whisper_binary,
};
use console_program_lifetime::let_go;
use console_response_times::{Wait, Waiting};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{FileType, FlockOperation, Mode, OFlags, flock, fstat, open};
use rustix::io::Errno;
use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Task {
  Fetch,
  Build,
  Dictate,
}

fn task(words: &[String]) -> Result<Task, ValidationError> {
  let line = console_core_arguments::read(&COMMAND, words)?;
  let Ok(fetch) = line.presence(FETCH);
  let Ok(build) = line.presence(BUILD);

  match (fetch, build) {
    (Presence::Present, Presence::Absent) => Ok(Task::Fetch),
    (Presence::Absent, Presence::Present) => Ok(Task::Build),
    (Presence::Absent, Presence::Absent) => Ok(Task::Dictate),
    (Presence::Present, Presence::Present) => {
      let Ok(refusal) = line.refusal(Reason::ExtraArgument(BUILD.spelling.to_string()));

      Err(refusal)
    }
  }
}

fn main() -> ExitCode {
  let asked: Vec<String> = std::env::args().skip(1).collect();
  let task = match task(&asked) {
    Ok(task) => task,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };

  match task {
    Task::Fetch => {
      match fetched() {
        Ok(()) => {}
        Err(why) => {
          let Ok(()) = report(
            "model",
            Failure {
              summary: "Couldn't download the language",
              body: &why.to_string(),
            },
          );
        }
      }

      match built() {
        Ok(()) => {}
        Err(why) => {
          let Ok(()) = report(
            "hearing",
            Failure {
              summary: "Couldn't set up dictation",
              body: &why.to_string(),
            },
          );
        }
      }

      ExitCode::SUCCESS
    }
    Task::Build => {
      match built() {
        Ok(()) => {}
        Err(why) => {
          let Ok(()) = report(
            "hearing",
            Failure {
              summary: "Couldn't set up dictation",
              body: &why.to_string(),
            },
          );
        }
      }

      ExitCode::SUCCESS
    }
    Task::Dictate => {
      match microphone() {
        Ok(Some(recorded)) => {
          let Ok(()) = wrote_down(recorded);
        }
        Ok(None) => {}
        Err(fault) => {
          let Ok(()) = report(
            "microphone",
            Failure {
              summary: "Couldn't finish the microphone request",
              body: &fault.to_string(),
            },
          );
        }
      }

      ExitCode::SUCCESS
    }
  }
}

struct Recorder {
  process: OwnedFd,
  press: u32,
}

struct Recorded {
  at: PathBuf,
  waited: Waiting,
}

fn microphone() -> io::Result<Option<Recorded>> {
  let Ok(note) = taking();
  let parent = note
    .parent()
    .ok_or_else(|| io::Error::other("nowhere to keep the recording"))?;

  let acquired = lock_directory(parent)?;
  let _held_during_transition = match acquired {
    Some(lock) => lock,
    None => {
      let Ok(()) = notify("The microphone is finishing the previous request", BRIEFLY);

      return Ok(None);
    }
  };

  let active = holder(&note)?;

  match active {
    Some(recorder) => {
      let Ok(mut waited) = Waiting::on(Wait {
        who: "console-dictate",
        what: "writing it down",
      });

      stop(&recorder.process)?;
      remove_file(&note)?;

      let Ok(()) = waited.mark("stopped");
      let Ok(at) = recording_path(recorder.press);

      Ok(Some(Recorded { at, waited }))
    }
    None => {
      listen(&note)?;

      Ok(None)
    }
  }
}

fn holder(at: &Path) -> io::Result<Option<Recorder>> {
  let Ok(held) = console_core_atomic_writes::read(at);
  let note = match held {
    Stored::Text(note) => note,
    Stored::Absent => return Ok(None),
    Stored::Failed(fault) => return Err(io::Error::other(fault)),
  };

  recorder_from_note(&note)
}

fn recorder_from_note(note: &str) -> io::Result<Option<Recorder>> {
  let Ok(told) = told_by(note);
  let (pid, press) = match told {
    Some((pid @ 1.., press)) => (pid, press),
    Some((..=0, _)) | None => return Ok(None),
  };
  let pid_2 =
    Pid::from_raw(pid).ok_or_else(|| io::Error::other("the recorder has no process ID"))?;
  let process = match pidfd_open(pid_2, PidfdFlags::empty()) {
    Ok(process) => process,
    Err(Errno::SRCH) => return Ok(None),
    Err(fault) => return Err(io::Error::from(fault)),
  };

  match note.split_whitespace().nth(2) {
    Some(inode) => {
      let recorded = inode.parse::<u64>().map_err(io::Error::other)?;
      let named = fstat(&process)?;

      match bool::then_some(named.st_ino == recorded, ()) {
        Some(()) => {}
        None => return Ok(None),
      }
    }
    None => {}
  }

  Ok(Some(Recorder { process, press }))
}

fn listen(note_at: &Path) -> io::Result<()> {
  let Ok(mut waiting) = Waiting::on(Wait {
    who: "console-dictate",
    what: "listening",
  });
  let press = std::process::id();
  let Ok(into) = recording_path(press);

  let Ok(arguments) = recording(&into);
  let (program, rest) = arguments
    .split_first()
    .ok_or_else(|| io::Error::other("No recording app is installed."))?;

  let mut starting = Command::new(program);
  starting
    .args(rest)
    .stdin(Stdio::null())
    .stdout(Stdio::null());

  let mut child = starting.spawn()?;
  let saved = save_recorder(&child, press, note_at);

  match saved {
    Ok(()) => {}
    Err(fault) => {
      match child.kill() {
        Ok(()) => match child.wait() {
          Ok(_ended) => {}
          Err(why) => eprintln!("console-dictate: reaping the unregistered recorder: {why}"),
        },
        Err(why) => eprintln!("console-dictate: stopping the unregistered recorder: {why}"),
      }

      return Err(fault);
    }
  }

  let Ok(()) = waiting.mark("microphone");
  let Ok(()) = notify("Listening", UNTIL_IT_CHANGES);
  let Ok(()) = waiting.finish();
  let Ok(()) = console_response_times::flush();

  Ok(())
}

fn save_recorder(child: &std::process::Child, press: u32, at: &Path) -> io::Result<()> {
  let process = pidfd_open(Pid::from_child(child), PidfdFlags::empty())?;
  let named = fstat(&process)?;
  let Ok(note) = serialize_note(Note {
    recorder: child.id(),
    press,
  });
  let identified = format!("{note} {}", named.st_ino);

  console_core_atomic_writes::whole(at, identified.as_bytes()).map_err(io::Error::other)
}

fn wrote_down(Recorded { at, mut waited }: Recorded) -> Result<(), Never> {
  let Ok(()) = read_out(&at, &mut waited);

  let Ok(()) = clean_file(&at);

  let Ok(()) = waited.finish();
  let Ok(()) = console_response_times::flush();

  Ok(())
}

fn read_out(recorded: &Path, waiting: &mut Waiting) -> Result<(), Never> {
  match fetched() {
    Ok(()) => {}
    Err(why) => {
      let Ok(()) = report(
        "model",
        Failure {
          summary: "Couldn't download the language",
          body: &why.to_string(),
        },
      );

      return Ok(());
    }
  }

  let Ok(()) = waiting.mark("model");
  let Ok(()) = notify("Transcribing…", UNTIL_IT_CHANGES);

  match transcribe(recorded) {
    Err(why) => {
      let Ok(()) = notify("Couldn't understand that", BRIEFLY);
      let Ok(()) = report(
        "hearing",
        Failure {
          summary: "Couldn't understand that",
          body: &why.to_string(),
        },
      );
    }
    Ok(words) => match ::core::primitive::bool::then_some(words.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = waiting.mark("heard");
        let Ok(()) = notify("No speech detected", BRIEFLY);
      }
      ::core::option::Option::None => {
        let Ok(()) = waiting.mark("heard");
        let Ok(letters) = console_core_number_conversion::fitted::<_, u64>(words.chars().count());
        let Ok(()) = waiting.counted("letters", letters);
        let Ok(()) = write(&words);
        let Ok(()) = waiting.mark("typed");
        let Ok(()) = notify(&words, BRIEFLY);
      }
    },
  }

  Ok(())
}

fn stop(process: &OwnedFd) -> io::Result<()> {
  match pidfd_send_signal(process, Signal::INT) {
    Ok(()) => {}
    Err(Errno::SRCH) => return Ok(()),
    Err(fault) => return Err(io::Error::from(fault)),
  }

  let mut watch = [PollFd::from_borrowed_fd(process.as_fd(), PollFlags::IN)];
  let _ready = poll(&mut watch, Some(&LEAVING))?;
  let ended = watch
    .first()
    .is_some_and(|ready| ready.revents().intersects(PollFlags::IN | PollFlags::HUP));

  match bool::then_some(ended, ()) {
    Some(()) => Ok(()),
    None => Err(io::Error::new(
      io::ErrorKind::TimedOut,
      "the microphone has not finished saving the recording",
    )),
  }
}

#[derive(Debug)]
enum DictationError {
  Machine(std::io::Error),
  NoModel,
  NoProgram(&'static str),
  CommandFailed(&'static str, std::process::ExitStatus),
  Transcript(std::string::FromUtf8Error),
  Nowhere,
}

impl std::fmt::Display for DictationError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      DictationError::Machine(fault) => write!(to, "{fault}"),
      DictationError::NoModel => write!(to, "nothing says where the model is kept"),
      DictationError::NoProgram(what) => write!(to, "there is no program to {what} with"),
      DictationError::CommandFailed(name, status) => write!(to, "{name} said no: {status}"),
      DictationError::Transcript(fault) => write!(to, "the transcription was not UTF-8: {fault}"),
      DictationError::Nowhere => write!(to, "nowhere to keep it"),
    }
  }
}

impl std::error::Error for DictationError {}

fn transcribe(recorded: &Path) -> Result<String, DictationError> {
  let wav = std::fs::read(recorded).map_err(DictationError::Machine)?;

  let Ok(heard) = detect_speech(&wav);

  match heard {
    VoiceActivity::Silence => return Ok(String::new()),
    VoiceActivity::Speech => {}
  }

  let Ok(engine) = engine();
  let Ok(model) = model();

  let model_3 = match model {
    Some(model_2) => model_2,
    None => return Err(DictationError::NoModel),
  };

  let Ok(language) = languages::current();
  let Ok(arguments) = whisper_arguments(&engine, &model_3, recorded, &language);

  let (program, rest) = match arguments.split_first() {
    Some((program, rest)) => (program, rest),
    None => return Err(DictationError::NoProgram("hear")),
  };

  let answered = Command::new(program)
    .args(rest)
    .stderr(Stdio::null())
    .output()
    .map_err(DictationError::Machine)?;

  match ::core::primitive::bool::then_some(answered.status.success(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      return Err(DictationError::CommandFailed(
        "whisper-cli",
        answered.status,
      ));
    }
  }

  let transcript = String::from_utf8(answered.stdout).map_err(DictationError::Transcript)?;
  let Ok(tidy) = tidy(&transcript);

  Ok(tidy)
}

fn write(words: &str) -> Result<(), Never> {
  let Ok(arguments) = typing(words);

  let (program, rest) = match arguments.split_first() {
    Some((program, rest)) => (program, rest),
    None => {
      let Ok(()) = report(
        "typing",
        Failure {
          summary: "Couldn't type the words",
          body: "No typing app is installed.",
        },
      );

      return Ok(());
    }
  };

  match Command::new(program).args(rest).status() {
    Err(why) => {
      let Ok(()) = report(
        "typing",
        Failure {
          summary: "Couldn't type the words",
          body: &why.to_string(),
        },
      );
    }
    Ok(status) => match ::core::primitive::bool::then_some(status.success(), ()) {
      ::core::option::Option::Some(()) => (),
      ::core::option::Option::None => {
        let Ok(()) = report(
          "typing",
          Failure {
            summary: "Couldn't type the words",
            body: &status.to_string(),
          },
        );
      }
    },
  }

  Ok(())
}

fn engine() -> Result<PathBuf, Never> {
  let Ok(ours) = whisper();

  match ours {
    Some(ours_2) => match artifact(&ours_2) {
      Ok(Artifact::Ready) => return Ok(ours_2),
      Ok(Artifact::Absent) => {}
      Err(fault) => eprintln!("console-dictate: opening {}: {fault}", ours_2.display()),
    },
    None => {}
  }

  let Ok(mut building) = InternalProgram::Dictate.command();
  building
    .arg(BUILD.spelling)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());

  let Ok(()) = background(&mut building, "starting the setup");

  let Ok(whisper_cli) = Program::WhisperCli.name();

  Ok(PathBuf::from(whisper_cli))
}

enum Artifact {
  Ready,
  Absent,
}

fn artifact(at: &Path) -> io::Result<Artifact> {
  let file = match open(at, OFlags::PATH | OFlags::CLOEXEC, Mode::empty()) {
    Ok(file) => file,
    Err(Errno::NOENT) => return Ok(Artifact::Absent),
    Err(fault) => return Err(io::Error::from(fault)),
  };
  let facts = fstat(&file)?;
  let ordinary = FileType::from_raw_mode(facts.st_mode) == FileType::RegularFile;

  match bool::then_some(ordinary, ()) {
    Some(()) => Ok(Artifact::Ready),
    None => Err(io::Error::new(
      io::ErrorKind::InvalidData,
      format!("{} is not a regular file", at.display()),
    )),
  }
}

fn lock_directory(at: &Path) -> io::Result<Option<OwnedFd>> {
  std::fs::create_dir_all(at)?;

  let directory = open(
    at,
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
    Mode::empty(),
  )?;

  match flock(&directory, FlockOperation::NonBlockingLockExclusive) {
    Ok(()) => Ok(Some(directory)),
    Err(Errno::AGAIN) => Ok(None),
    Err(fault) => Err(io::Error::from(fault)),
  }
}

fn missing_is_gone(fault: io::Error) -> io::Result<()> {
  match bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
    Some(()) => Ok(()),
    None => Err(fault),
  }
}

fn remove_file(at: &Path) -> io::Result<()> {
  std::fs::remove_file(at).or_else(missing_is_gone)
}

fn remove_tree(at: &Path) -> io::Result<()> {
  std::fs::remove_dir_all(at).or_else(missing_is_gone)
}

fn clean_file(at: &Path) -> Result<(), Never> {
  match remove_file(at) {
    Ok(()) => {}
    Err(fault) => eprintln!("console-dictate: removing {}: {fault}", at.display()),
  }

  Ok(())
}

#[derive(Debug)]
enum BuildError {
  NoWhisper,
  Nowhere,
  Machine(std::io::Error),
  NoStep,
  NoProgram(OsString, std::io::Error),
  CommandFailed(OsString, std::process::ExitStatus),
}

impl std::fmt::Display for BuildError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      BuildError::NoWhisper => write!(to, "nothing says where whisper is kept"),
      BuildError::Nowhere => write!(to, "nowhere to keep it"),
      BuildError::Machine(fault) => write!(to, "{fault}"),
      BuildError::NoStep => write!(to, "a step of the build named no program to run"),
      BuildError::NoProgram(program, fault) => {
        write!(to, "{} could not be run: {fault}", program.display())
      }
      BuildError::CommandFailed(program, status) => {
        write!(to, "{} said no: {status}", program.display())
      }
    }
  }
}

impl std::error::Error for BuildError {}

fn built() -> Result<(), BuildError> {
  let Ok(path) = whisper();
  let ours = match path {
    Some(path_2) => path_2,
    None => return Err(BuildError::NoWhisper),
  };

  let present = artifact(&ours).map_err(BuildError::Machine)?;

  match present {
    Artifact::Ready => return Ok(()),
    Artifact::Absent => {}
  }

  let parent = match ours.parent() {
    Some(parent) => parent,
    None => return Err(BuildError::Nowhere),
  };

  let acquired = lock_directory(parent).map_err(BuildError::Machine)?;
  let _held_during_build = match acquired {
    Some(lock) => lock,
    None => return Ok(()),
  };
  let present_after_lock = artifact(&ours).map_err(BuildError::Machine)?;

  match present_after_lock {
    Artifact::Ready => Ok(()),
    Artifact::Absent => build(&ours),
  }
}

fn build(ours: &Path) -> Result<(), BuildError> {
  let Ok(at) = making();

  remove_tree(&at).map_err(BuildError::Machine)?;

  match at.parent() {
    Some(parent) => std::fs::create_dir_all(parent).map_err(BuildError::Machine)?,
    None => {}
  }

  let Ok(()) = notify("Setting up dictation…", UNTIL_IT_CHANGES);
  let prepared = prepare_engine(&at, ours);

  match remove_tree(&at) {
    Ok(()) => {}
    Err(fault) => eprintln!("console-dictate: removing {}: {fault}", at.display()),
  }

  prepared?;

  let Ok(()) = notify("Dictation is ready", BRIEFLY);

  Ok(())
}

fn prepare_engine(at: &Path, ours: &Path) -> Result<(), BuildError> {
  let Ok(clone_command) = cloning(at);
  let Ok(configure_command) = configuring(at);
  let Ok(compile_command) = compiling(at);

  for arguments in [clone_command, configure_command, compile_command] {
    let (program, rest) = match arguments.split_first() {
      Some((program, rest)) => (program, rest),
      None => return Err(BuildError::NoStep),
    };

    let answered = Command::new(program)
      .args(rest)
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .status()
      .map_err(|why| BuildError::NoProgram(program.clone(), why))?;

    match ::core::primitive::bool::then_some(answered.success(), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        return Err(BuildError::CommandFailed(program.clone(), answered));
      }
    }
  }

  let coming = ours.with_extension("coming");
  let Ok(made) = whisper_binary(at);
  let stage =
    std::fs::copy(made, &coming).and_then(|_copied_bytes| std::fs::rename(&coming, ours));

  match stage {
    Ok(()) => Ok(()),
    Err(fault) => {
      let Ok(()) = clean_file(&coming);

      Err(BuildError::Machine(fault))
    }
  }
}

fn fetched() -> Result<(), DictationError> {
  let Ok(model) = model();

  let model_3 = match model {
    Some(model_2) => model_2,
    None => return Err(DictationError::NoModel),
  };

  let present = artifact(&model_3).map_err(DictationError::Machine)?;

  match present {
    Artifact::Ready => return Ok(()),
    Artifact::Absent => {}
  }

  let parent = match model_3.parent() {
    Some(parent) => parent,
    None => return Err(DictationError::Nowhere),
  };

  let acquired = lock_directory(parent).map_err(DictationError::Machine)?;
  let _held_during_download = match acquired {
    Some(lock) => lock,
    None => {
      return Err(DictationError::Machine(io::Error::new(
        io::ErrorKind::WouldBlock,
        "another dictation setup request is running",
      )));
    }
  };
  let present_after_lock = artifact(&model_3).map_err(DictationError::Machine)?;

  match present_after_lock {
    Artifact::Ready => Ok(()),
    Artifact::Absent => {
      let Ok(()) = notify("Downloading the language…", UNTIL_IT_CHANGES);
      let coming = parent.join("coming.bin");
      let downloaded = download(&model_3, &coming);

      match downloaded {
        Ok(()) => Ok(()),
        Err(fault) => {
          let Ok(()) = clean_file(&coming);

          Err(fault)
        }
      }
    }
  }
}

fn download(model: &Path, coming: &Path) -> Result<(), DictationError> {
  let Ok(arguments) = fetching(coming);
  let (program, rest) = match arguments.split_first() {
    Some((program, rest)) => (program, rest),
    None => return Err(DictationError::NoProgram("fetch the words")),
  };

  let answered = Command::new(program)
    .args(rest)
    .status()
    .map_err(DictationError::Machine)?;

  match ::core::primitive::bool::then_some(answered.success(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      return Err(DictationError::CommandFailed("curl", answered));
    }
  }

  std::fs::rename(coming, model).map_err(DictationError::Machine)
}

const LEAVING: Timespec = Timespec {
  tv_sec: 2,
  tv_nsec: 0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Until(u32);

const UNTIL_IT_CHANGES: Until = Until(0);

const BRIEFLY: Until = Until(2_000);

fn notify(what: &str, until: Until) -> Result<(), Never> {
  let Ok(mut saying) = Program::NotifySend.command();

  saying
    .args([
      "--app-name=Console",
      "--urgency=low",
      &format!("--expire-time={}", until.0),
      "--hint=string:x-canonical-private-synchronous:console-dictate",
      "--icon=audio-input-microphone",
      "--",
      what,
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null());

  background(&mut saying, "showing a notification")
}

fn background(command: &mut Command, doing: &str) -> Result<(), Never> {
  match let_go(command) {
    Ok(_runs_after_this_press) => {}
    Err(fault) => eprintln!("console-dictate: {doing}: {fault}"),
  }

  Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Failure<'a> {
  summary: &'a str,
  body: &'a str,
}

fn report(kind: &str, fault: Failure<'_>) -> Result<(), Never> {
  let Failure { summary, body } = fault;

  eprintln!("console-dictate: {summary}: {body}");

  let Ok(mut saying) = InternalProgram::ConsoleSay.command();
  saying.args([kind, summary, body]).stdin(Stdio::null());

  background(&mut saying, "showing a microphone error")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn recorder_notes_accept_the_existing_format_and_refuse_a_reused_identity() -> io::Result<()> {
    let pid = rustix::process::getpid();
    let process = pidfd_open(pid, PidfdFlags::empty())?;
    let named = fstat(&process)?;
    let legacy = format!("{} 42", pid.as_raw_pid());
    let matching = format!("{legacy} {}", named.st_ino);
    let different = format!("{legacy} {}", named.st_ino ^ 1);
    let old = recorder_from_note(&legacy)?;
    let current = recorder_from_note(&matching)?;
    let reused = recorder_from_note(&different)?;
    let not_a_process = recorder_from_note("-1 42")?;

    assert!(old.is_some_and(|one| one.press == 42));
    assert!(current.is_some_and(|one| one.press == 42));
    assert!(reused.is_none());
    assert!(not_a_process.is_none());

    Ok(())
  }
}
