//! Password checks for an already logged-in account.
//! The bundled backend can disclose password status without reading a secret.
//! An administrator-installed alternative owns its account policy.

use std::fs::File;
use std::io::{self, Seek, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::{const_u16_index, fitted, index};
use console_core_text_boundaries::previous_word_start;
use console_login_session::{Request, Rules, authenticate};
use console_operating_system_linux::program_lifetime::TimedReadPipe;
use rustix::fs::{
  AtFlags, FileType, MemfdFlags, Mode, OFlags, SealFlags, fcntl_add_seals, memfd_create, open,
  statat,
};
use rustix::process::{DumpableBehavior, set_dumpable_behavior};

use crate::authentication::{PAM_DIRECTORY, PamService, worker_path};
use crate::fingerprint::{Worker, session_account};
use crate::{Availability, Verification};

pub const WORKER: &str = "--password-worker";
const LIMIT: u16 = 16_384;
const MAX_READ: u16 = LIMIT.saturating_add(1);

const UNIX_LIMIT: u32 = 511;
const MASK_CHARACTERS: u32 = 24;

pub struct Password {
  bytes: Vec<u8>,
  overflow: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
  Empty,
  Entered,
}

impl Default for Password {
  fn default() -> Self {
    Self {
      bytes: Vec::with_capacity(
        const {
          match const_u16_index(LIMIT) {
            Ok(capacity) => capacity,
          }
        },
      ),
      overflow: 0,
    }
  }
}

impl Password {
  pub fn push(&mut self, key: char) -> Result<(), Never> {
    match { ::core::primitive::bool::then_some(key.is_control(), ()) } {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let mut encoded = [0; 4];
    let text = key.encode_utf8(&mut encoded);
    let Ok(incoming) = fitted::<_, u32>(text.len());
    let Ok(entered) = fitted::<_, u32>(self.bytes.len());

    match {
      ::core::primitive::bool::then_some(
        self.overflow == 0 && incoming <= u32::from(LIMIT).saturating_sub(entered),
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        self.bytes.extend_from_slice(text.as_bytes());
      }
      ::core::option::Option::None => {
        self.overflow = self.overflow.saturating_add(1);
      }
    }

    std::hint::black_box(&mut encoded).fill(0);
    Ok(())
  }

  pub fn erase(&mut self) -> Result<(), Never> {
    match self.overflow {
      0 => {}
      _ => {
        self.overflow = self.overflow.saturating_sub(1);
        return Ok(());
      }
    }

    let text = match std::str::from_utf8(&self.bytes) {
      Ok(text) => text,
      Err(_invalid_text) => return Ok(()),
    };
    let character = match text.chars().next_back() {
      Some(last) => last,
      None => return Ok(()),
    };
    let Ok(length) = fitted::<_, u32>(self.bytes.len());
    let Ok(width) = fitted::<_, u32>(character.len_utf8());
    self.erase_from(u64::from(length.saturating_sub(width)))
  }

  pub fn erase_previous_word(&mut self) -> Result<(), Never> {
    match self.overflow {
      0 => {}
      _ => return self.erase(),
    }

    let text = match std::str::from_utf8(&self.bytes) {
      Ok(text) => text,
      Err(_invalid_text) => return Ok(()),
    };
    let Ok(from) = previous_word_start(text);

    self.erase_from(from)
  }

  fn erase_from(&mut self, from: u64) -> Result<(), Never> {
    let Ok(start) = index(from);

    match self.bytes.get_mut(start..) {
      Some(erased) => {
        std::hint::black_box(erased).fill(0);
        self.bytes.truncate(start);
      }
      None => {}
    }

    Ok(())
  }

  pub fn clear(&mut self) -> Result<(), Never> {
    std::hint::black_box(self.bytes.as_mut_slice()).fill(0);
    self.bytes.clear();
    self.overflow = 0;

    Ok(())
  }

  #[must_use]
  pub fn entry(&self) -> Result<Entry, Never> {
    Ok(match self.bytes.as_slice() {
      [] => Entry::Empty,
      [_, ..] => Entry::Entered,
    })
  }

  #[must_use]
  pub fn mask(&self) -> Result<String, Never> {
    let Ok(shown) = index(MASK_CHARACTERS);
    Ok(std::str::from_utf8(&self.bytes).map_or_else(
      |_| String::new(),
      |text| text.chars().take(shown).map(|_| '•').collect(),
    ))
  }

  fn check_length(&self, limit: u32) -> Result<(), CheckError> {
    let Ok(length) = fitted::<_, u32>(self.bytes.len());

    match {
      let _explicit_ordering = ::core::cmp::Ord::cmp(
        &::core::convert::identity(self.overflow),
        &::core::convert::identity(0),
      );
      _explicit_ordering
    } {
      ::core::cmp::Ordering::Greater => {
        return Err(CheckError::InputTooLong);
      }
      ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal => {}
    }

    match { ::core::primitive::bool::then_some(length > limit, ()) } {
      ::core::option::Option::Some(()) => {
        return Err(CheckError::SystemLimit);
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }
}

impl Drop for Password {
  fn drop(&mut self) {
    let Ok(()) = self.clear();
  }
}

pub struct Check(Worker);

#[derive(Debug, PartialEq, Eq)]
pub enum CheckError {
  InputTooLong,
  SystemLimit,
  Unavailable,
}

impl Check {
  pub fn start(password: Password) -> Result<Self, CheckError> {
    let Ok(configured) = rules();
    let (service, limit) = configured.ok_or(CheckError::Unavailable)?;
    password.check_length(limit)?;
    let Ok(service_name) = service.name();

    match Self::spawn(&password, service_name) {
      Ok(check) => Ok(check),
      Err(fault) => {
        console_core_logging::warn!("cannot start password check: {fault}");
        Err(CheckError::Unavailable)
      }
    }
  }

  fn spawn(password: &Password, service: &str) -> io::Result<Self> {
    let mut command = launcher()?;
    let executable = worker_path()?;
    let input = sealed_input(password)?;
    command
      .args([
        "--user",
        "--quiet",
        "--wait",
        "--collect",
        "--pipe",
        "--property=RuntimeMaxSec=20s",
        "--property=LimitCORE=0",
        "--",
      ])
      .arg(executable)
      .arg(WORKER)
      .arg(service)
      .stdin(Stdio::from(input))
      .stdout(Stdio::null());
    Worker::spawn(&mut command).map(Self)
  }

  pub fn result(&mut self) -> Result<Verification, Never> {
    self.0.result()
  }
}

fn launcher() -> io::Result<Command> {
  const LAUNCHER: &str = {
    let Ok(program) = Program::SystemdRun.name();
    program
  };

  for directory in ["/usr/bin", "/run/current-system/sw/bin"] {
    let found = open(
      directory,
      OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
      Mode::empty(),
    )
    .and_then(|parent| statat(parent, LAUNCHER, AtFlags::empty()));

    match found {
      Ok(metadata) => {
        match (FileType::from_raw_mode(metadata.st_mode) == FileType::RegularFile).then_some(()) {
          Some(()) => {
            let Ok(command) = Program::SystemdRun.command_from(Path::new(directory));
            return Ok(command);
          }
          None => {}
        }
      }
      Err(_not_available) => continue,
    }
  }

  Err(io::Error::from(io::ErrorKind::NotFound))
}

fn sealed_input(password: &Password) -> io::Result<File> {
  let flags = MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING;
  let handle = memfd_create("console-lock-input", flags)?;
  let mut input = File::from(handle);
  input.write_all(&password.bytes)?;
  input.rewind()?;
  let seals = SealFlags::SEAL | SealFlags::WRITE | SealFlags::SHRINK | SealFlags::GROW;
  fcntl_add_seals(&input, seals)?;
  Ok(input)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn word_deletion_uses_the_same_boundaries_in_passwords() {
    let mut password = Password::default();

    for letter in "one cafe\u{301}  ".chars() {
      let Ok(()) = password.push(letter);
    }

    let Ok(()) = password.erase_previous_word();
    assert_eq!(password.bytes, b"one ");
  }

  #[test]
  fn long_unicode_passwords_are_kept_whole_and_rejected_by_the_legacy_backend() {
    let mut password = Password::default();

    for _ in 0..1024_u32 {
      let Ok(()) = password.push('😀');
    }

    let Ok(length) = fitted::<_, u32>(password.bytes.len());

    assert_eq!(length, 4096);
    assert_eq!(password.check_length(u32::from(LIMIT)), Ok(()));
    assert_eq!(
      password.check_length(UNIX_LIMIT),
      Err(CheckError::SystemLimit)
    );
    let Ok(()) = password.erase();
    let Ok(after_erase) = fitted::<_, u32>(password.bytes.len());

    assert_eq!(after_erase, 4092);
    assert!(
      password
        .bytes
        .chunks_exact(4)
        .all(|key| key == "😀".as_bytes())
    );
  }

  #[test]
  fn overflow_never_authenticates_a_shortened_password() {
    let mut password = Password::default();

    for _ in 0..LIMIT {
      let Ok(()) = password.push('x');
    }

    let Ok(()) = password.push('y');
    assert_eq!(
      password.check_length(u32::from(LIMIT)),
      Err(CheckError::InputTooLong)
    );
    let Ok(()) = password.erase();
    assert_eq!(password.check_length(u32::from(LIMIT)), Ok(()));
  }

  #[test]
  fn the_password_handoff_cannot_be_changed() -> io::Result<()> {
    let mut password = Password::default();
    let Ok(()) = password.push('😀');
    let mut input = sealed_input(&password)?;

    let truncate = input.set_len(0);
    let overwrite = input.write_all(b"?");
    assert!(matches!(truncate, Err(fault) if fault.kind() == io::ErrorKind::PermissionDenied));
    assert!(matches!(overwrite, Err(fault) if fault.kind() == io::ErrorKind::PermissionDenied));
    let mut encoded = [0_u8; 4];
    let file = TimedReadPipe::new(OwnedFd::from(input))?;
    let deadline = Instant::now()
      .checked_add(Duration::from_secs(1))
      .ok_or(io::ErrorKind::TimedOut)?;
    let received = file.read_until(&mut encoded, deadline, Instant::now)?;
    assert_eq!(received, "😀".as_bytes());
    Ok(())
  }
}

impl AsFd for Check {
  fn as_fd(&self) -> BorrowedFd<'_> {
    self.0.as_fd()
  }
}

#[must_use]
pub fn enable_for_lock() -> Result<Availability, Never> {
  let Ok(configured) = rules();

  Ok(match configured {
    Some(_) => {
      let Ok(protection) = protect_process();

      match protection {
        Protection::Protected => Availability::Available,
        Protection::Unavailable => Availability::Unavailable,
      }
    }
    None => Availability::Unavailable,
  })
}

pub fn configured_for_session() -> Result<Availability, Never> {
  let Ok(configured) = rules();

  Ok(match configured {
    Some((PamService::LongPassword, _)) => Availability::Available,
    Some((PamService::Password, _)) => match account_password_status() {
      Ok(availability) => availability,
      Err(fault) => {
        console_core_logging::debug!("cannot check account password status: {fault}");
        Availability::Unavailable
      }
    },
    Some((PamService::Fingerprint, _)) | None => Availability::Unavailable,
  })
}

fn account_password_status() -> io::Result<Availability> {
  let mut command = Program::Passwd.provide()?;
  let output = command
    .env_clear()
    .env("LC_ALL", "C")
    .arg("--status")
    .output()?;
  let field = output
    .stdout
    .split(|byte| byte.is_ascii_whitespace())
    .filter(|part| !part.is_empty())
    .nth(1);

  Ok(match (output.status.success(), field) {
    (true, Some(b"P" | b"PS")) => Availability::Available,
    (true, Some(_) | None) | (false, _) => Availability::Unavailable,
  })
}

fn rules() -> Result<Option<(PamService, u32)>, Never> {
  Ok(
    [
      (PamService::LongPassword, u32::from(LIMIT)),
      (PamService::Password, UNIX_LIMIT),
    ]
    .into_iter()
    .find(|(service, _)| {
      let Ok(availability) = service.availability();
      availability == Availability::Available
    }),
  )
}

enum Protection {
  Protected,
  Unavailable,
}

fn protect_process() -> Result<Protection, Never> {
  Ok(match set_dumpable_behavior(DumpableBehavior::NotDumpable) {
    Ok(()) => Protection::Protected,
    Err(fault) => {
      console_core_logging::warn!("cannot protect password memory: {fault}");
      Protection::Unavailable
    }
  })
}

#[must_use]
pub fn work(
  service: &str,
  login_name: Option<&str>,
  input: OwnedFd,
  clock: impl Fn() -> Instant,
) -> Result<ExitCode, Never> {
  let Ok(protection) = protect_process();

  match protection {
    Protection::Protected => {}
    Protection::Unavailable => return Ok(ExitCode::FAILURE),
  }

  let (configured, limit) = {
    let Ok(optional_configured) = rules();

    match optional_configured {
      Some((matched_configured, matched_limit)) => (matched_configured, matched_limit),
      ::core::option::Option::None => return Ok(ExitCode::FAILURE),
    }
  };
  let Ok(service_name) = configured.name();

  match { ::core::primitive::bool::then_some(service != service_name, ()) } {
    ::core::option::Option::Some(()) => {
      return Ok(ExitCode::FAILURE);
    }
    ::core::option::Option::None => {}
  }

  let person = {
    let Ok(optional_person) = session_account(login_name);

    match optional_person {
      Some(matched_person) => matched_person,
      ::core::option::Option::None => return Ok(ExitCode::FAILURE),
    }
  };

  let mut password = Password {
    bytes: vec![
      0;
      const {
        match const_u16_index(MAX_READ) {
          Ok(capacity) => capacity,
        }
      }
    ],
    overflow: 0,
  };

  let received = read_password(input, &mut password.bytes, clock);
  let entered = match received {
    Ok(bytes) => bytes,
    Err(fault) => {
      console_core_logging::debug!("could not read password input: {fault}");
      return Ok(ExitCode::FAILURE);
    }
  };
  let Ok(cutoff) = index(entered);
  password.bytes.truncate(cutoff);

  match { ::core::primitive::bool::then_some(!(1..=limit).contains(&entered), ()) } {
    ::core::option::Option::Some(()) => {
      return Ok(ExitCode::FAILURE);
    }
    ::core::option::Option::None => {}
  }

  let null = match crate::system::null_input() {
    Ok(null) => null,
    Err(fault) => {
      console_core_logging::debug!("could not detach password input: {fault}");
      return Ok(ExitCode::FAILURE);
    }
  };

  match rustix::stdio::dup2_stdin(null) {
    Ok(()) => {}
    Err(fault) => {
      console_core_logging::debug!("could not replace password input: {fault}");
      return Ok(ExitCode::FAILURE);
    }
  }

  let secret = {
    let outcome_secret = std::str::from_utf8(&password.bytes);

    match outcome_secret {
      Ok(matched_secret) => matched_secret,
      Err(_invalid_password_encoding) => return Ok(ExitCode::FAILURE),
    }
  };

  let request = Request {
    service: service_name,
    person,
    rules: Rules::Directory(Path::new(PAM_DIRECTORY)),
  };

  Ok(match authenticate(request, secret) {
    Ok(_) => ExitCode::SUCCESS,
    Err(_authentication_refused) => ExitCode::FAILURE,
  })
}

fn read_password(
  input: OwnedFd,
  bytes: &mut [u8],
  clock: impl Fn() -> Instant,
) -> io::Result<u32> {
  const WAIT_FOR_INPUT: Duration = Duration::from_secs(5);
  let deadline = clock()
    .checked_add(WAIT_FOR_INPUT)
    .ok_or(io::ErrorKind::TimedOut)?;
  let pipe = TimedReadPipe::new(input)?;
  let filled = pipe.read_until(bytes, deadline, clock)?;
  let Ok(received) = fitted::<_, u32>(filled.len());
  Ok(received)
}
