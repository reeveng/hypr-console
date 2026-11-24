use std::collections::{BTreeMap, btree_map::Entry};
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
use std::path::Path;

use console_core_state_machine::Never;
use rustix::fs::{Mode, OFlags};

use console_stream_privacy::{Obs, Output};

const READ_LIMIT: u32 = 1024 * 1024;

#[derive(Default)]
pub struct Logs {
  progress: BTreeMap<(u64, u64), Progress>,
}

impl Logs {
  pub fn read(&mut self, proc: &Path, uid: u32) -> io::Result<Obs> {
    let processes = fs::read_dir(proc)?;
    let mut reading = Obs::CLOSED;
    let mut previous = std::mem::take(&mut self.progress);

    for process in processes.flatten() {
      let path = process.path();
      let Ok(kind) = process_kind(&path, uid);

      let Ok(observed) = match kind {
        Process::Obs => self.read_process(&path, &mut previous),
        Process::Other => continue,
      };
      let Ok(combined) = combine(reading, observed);
      reading = combined;
    }

    Ok(reading)
  }

  fn read_process(
    &mut self,
    process: &Path,
    previous: &mut BTreeMap<(u64, u64), Progress>,
  ) -> Result<Obs, Never> {
    let fds = match fs::read_dir(process.join("fd")) {
      Ok(fds) => fds,
      Err(_descriptors_unavailable) => return Ok(Obs::UNREADABLE),
    };

    let mut reading = None;

    for fd in fds.flatten() {
      let (file, metadata) = match open_log(&fd.path()) {
        Ok(Some(log)) => log,
        Ok(None) => continue,
        Err(_descriptor_unavailable) => continue,
      };

      let key = (metadata.dev(), metadata.ino());
      let progress = match self.progress.entry(key) {
        Entry::Occupied(entry) => entry.into_mut(),
        Entry::Vacant(entry) => {
          let saved_progress = match previous.remove(&key) {
            Some(saved) => saved,
            None => Progress::default(),
          };
          entry.insert(saved_progress)
        }
      };
      let observed = match progress.read(file, metadata.len()) {
        Ok(observed) => observed,
        Err(_log_unreadable) => Obs::UNREADABLE,
      };
      let combined = match reading {
        Some(earlier) => {
          let Ok(merged) = combine(earlier, observed);
          merged
        }
        None => observed,
      };
      reading = Some(combined);
    }

    Ok(match reading {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => Obs::UNREADABLE,
    })
  }
}

fn combine(first: Obs, second: Obs) -> Result<Obs, Never> {
  let Ok(streaming) = combine_output(first.streaming, second.streaming);
  let Ok(recording) = combine_output(first.recording, second.recording);

  Ok(Obs {
    streaming,
    recording,
  })
}

fn combine_output(first: Output, second: Output) -> Result<Output, Never> {
  Ok(match (first, second) {
    (Output::Active, _) | (_, Output::Active) => Output::Active,
    (Output::Unreadable, _) | (_, Output::Unreadable) => Output::Unreadable,
    (Output::Idle, _) | (_, Output::Idle) => Output::Idle,
    (Output::Closed, Output::Closed) => Output::Closed,
  })
}

enum Process {
  Obs,
  Other,
}

fn process_kind(process: &Path, uid: u32) -> Result<Process, Never> {
  let is_obs = process
    .file_name()
    .is_some_and(|name| name.as_bytes().iter().all(u8::is_ascii_digit))
    && fs::metadata(process).is_ok_and(|metadata| metadata.uid() == uid)
    && fs::read(process.join("comm")).is_ok_and(|name| name == b"obs\n")
    && fs::read(process.join("stat")).is_ok_and(|stat| {
      stat.rsplit(|byte| *byte == b')').next().is_some_and(
        |tail| matches!(tail.get(1), Some(state) if !matches!(state, b'Z' | b'X' | b'x')),
      )
    });

  Ok(match is_obs.then_some(()) {
    Some(()) => Process::Obs,
    None => Process::Other,
  })
}

fn open_log(fd: &Path) -> io::Result<Option<(File, fs::Metadata)>> {
  let target = fs::read_link(fd)?;
  let name = target.as_os_str().as_bytes();
  let log_name = match name.strip_suffix(b" (deleted)") {
    Some(before_deleted) => before_deleted,
    None => name,
  };
  let path = Path::new(OsStr::from_bytes(log_name));

  match {
    ::core::primitive::bool::then_some(
      path.extension() != Some(OsStr::new("txt"))
        || !path
          .parent()
          .is_some_and(|dir| dir.ends_with("obs-studio/logs")),
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let link = fs::symlink_metadata(fd)?;

  match link.mode() & 0o200 {
    0 => return Ok(None),
    _writable => {}
  }

  let opened = rustix::fs::open(
    fd,
    OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
    Mode::empty(),
  )?;
  let file = File::from(opened);
  let metadata = file.metadata()?;

  Ok(metadata.is_file().then_some((file, metadata)))
}

#[derive(Clone, Copy, Default)]
struct Progress {
  offset: u64,
  streaming: Activity,
  recording: Activity,
  skipping_line: bool,
}

#[derive(Clone, Copy, Default)]
enum Activity {
  #[default]
  Idle,
  Active,
}

impl Activity {
  fn output(self) -> Result<Output, Never> {
    Ok(match self {
      Self::Idle => Output::Idle,
      Self::Active => Output::Active,
    })
  }
}

enum Mark {
  Streaming(Activity),
  Recording(Activity),
}

impl Progress {
  fn read(&mut self, mut file: impl Read + Seek, length: u64) -> io::Result<Obs> {
    match length.cmp(&self.offset) {
      ::core::cmp::Ordering::Less => {
        *self = Self::default();
      }
      ::core::cmp::Ordering::Equal | ::core::cmp::Ordering::Greater => {}
    }

    file.seek(SeekFrom::Start(self.offset))?;
    let bytes = console_core_bounded_reads::bytes(file.take(u64::from(READ_LIMIT)), READ_LIMIT)?;
    let read = u32::try_from(bytes.len()).map_err(io::Error::other)?;

    let mut complete_bytes = 0_u64;

    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
      match { ::core::primitive::bool::then_some(!line.ends_with(b"\n"), ()) } {
        ::core::option::Option::Some(()) => {
          break;
        }
        ::core::option::Option::None => {}
      }

      match { ::core::primitive::bool::then_some(!self.skipping_line, ()) } {
        ::core::option::Option::Some(()) => {
          let Ok(mark) = marker(line);

          match mark {
            Some(Mark::Streaming(state)) => self.streaming = state,
            Some(Mark::Recording(state)) => self.recording = state,
            None => {}
          }
        }
        ::core::option::Option::None => {}
      }

      self.skipping_line = false;
      let line_length = u64::try_from(line.len()).map_err(io::Error::other)?;
      complete_bytes = complete_bytes.wrapping_add(line_length);
    }

    match (read, complete_bytes) {
      (READ_LIMIT, 0) => {
        self.offset = self.offset.wrapping_add(u64::from(read));
        self.skipping_line = true;
      }
      (_, _) => {
        self.offset = self.offset.wrapping_add(complete_bytes);
      }
    }

    let Ok(streaming) = self.streaming.output();
    let Ok(recording) = self.recording.output();

    Ok(match (read == READ_LIMIT).then_some(()) {
      Some(()) => Obs::UNREADABLE,
      None => Obs {
        streaming,
        recording,
      },
    })
  }
}

fn marker(line: &[u8]) -> Result<Option<Mark>, Never> {
  let message = match line.get(12..) {
    Some(message) => message,
    None => return Ok(None),
  };

  Ok(
    [
      (
        b": ==== Streaming Start =".as_slice(),
        Mark::Streaming(Activity::Active),
      ),
      (
        b": ==== Streaming Stop =".as_slice(),
        Mark::Streaming(Activity::Idle),
      ),
      (
        b": ==== Recording Start =".as_slice(),
        Mark::Recording(Activity::Active),
      ),
      (
        b": ==== Recording Stop =".as_slice(),
        Mark::Recording(Activity::Idle),
      ),
    ]
    .into_iter()
    .find_map(|(prefix, mark)| message.starts_with(prefix).then_some(mark)),
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use Output::{Active, Idle};
  use console_core_atomic_writes::whole;
  use std::io::Cursor;

  #[test]
  fn complete_markers_track_each_output_independently() -> io::Result<()> {
    let mut log = Progress::default();
    let mut bytes = b"00:00:00.000: ==== Recording Start ======\n".to_vec();
    let read = |log_2: &mut Progress, bytes_2: &[u8]| {
      let length = bytes_2.len().try_into().map_err(io::Error::other)?;
      log_2.read(Cursor::new(bytes_2), length)
    };

    for (appended, streaming, recording) in [
      (b"".as_slice(), Idle, Active),
      (
        b"00:00:01.000: ==== Streaming Start ===".as_slice(),
        Idle,
        Active,
      ),
      (b"===\n".as_slice(), Active, Active),
      (
        b"00:00:02.000: warning: ==== Streaming Stop ====\n".as_slice(),
        Active,
        Active,
      ),
      (
        b"00:00:03.000: ==== Streaming Stop ======\n".as_slice(),
        Idle,
        Active,
      ),
      (
        b"00:00:04.000: ==== Recording Stop ======\n".as_slice(),
        Idle,
        Idle,
      ),
    ] {
      bytes.extend(appended);
      let observed = read(&mut log, &bytes)?;
      assert_eq!(
        observed,
        Obs {
          streaming,
          recording
        }
      );
    }

    Ok(())
  }

  #[test]
  fn a_stale_log_has_no_effect_without_a_running_obs_holding_it()
  -> Result<(), Box<dyn std::error::Error>> {
    let root = console_core_temporary_directories::fresh("stream-privacy")?;
    let proc = root.join("proc");
    let running = proc.join("1234");
    let log_dir = root.join("obs-studio/logs");
    let log = log_dir.join("current.txt");
    fs::create_dir_all(running.join("fd"))?;
    fs::create_dir_all(&log_dir)?;
    whole(&running.join("comm"), b"obs\n")?;
    whole(&running.join("stat"), b"1234 (obs) S 1 2\n")?;
    whole(&log, b"00:00:01.000: ==== Streaming Start ======\n")?;
    std::os::unix::fs::symlink(log, running.join("fd/11"))?;
    std::os::unix::fs::symlink(log_dir.join("rotated.txt"), running.join("fd/12"))?;
    let mut logs = Logs::default();
    let uid = rustix::process::getuid().as_raw();

    let read = logs.read(&proc, uid)?;
    assert_eq!(
      read,
      Obs {
        streaming: Active,
        recording: Idle
      }
    );
    whole(&running.join("stat"), b"1234 (obs) Z 1 2\n")?;
    let logs_read = logs.read(&proc, uid)?;
    assert_eq!(logs_read, Obs::CLOSED);

    Ok(())
  }
}
