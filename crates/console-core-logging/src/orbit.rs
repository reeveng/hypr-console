//! An opted-in host keeps severe reports locally until Orbit acknowledges them.
//! `Outbox::send_pending` keeps each report until the transport acknowledges
//! it. Hosts with a read-only home can queue reports in the runtime directory
//! for this pass to copy.

use console_core_atomic_writes::{Unwritten, read_bytes, whole_without_overwriting};
use console_core_bytes::hex::{self, Case};
use console_core_never::Never;
use console_core_places::Base;
use core::fmt;
use core::mem::MaybeUninit;
use rustix::fs::{FlockOperation, flock};
use rustix::rand::{GetRandomFlags, getrandom};
use serde_json::json;
use std::fs::{self, DirBuilder, File};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::string::{String, ToString};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

const DSN: &str = "ORBIT_ERROR_DSN";
const SLOTS: u32 = 128;
const MESSAGE_CHARS: u16 = 8_192;

struct OutboxName(String);

impl AsRef<Path> for OutboxName {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

pub struct Destination {
  endpoint: Url,
  key: String,
  namespace: OutboxName,
}

impl Destination {
  #[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    expect(
      explicit026_env_read_once,
      reason = "this is the configuration boundary for the Orbit reporting destination"
    )
  )]
  pub fn configured() -> io::Result<Option<Self>> {
    match std::env::var(DSN) {
      Ok(value) => match value.trim() {
        "" => Ok(None),
        dsn => Self::parse(dsn).map(Some),
      },
      Err(std::env::VarError::NotPresent) => Ok(None),
      Err(std::env::VarError::NotUnicode(_)) => invalid_dsn(),
    }
  }

  fn parse(dsn: &str) -> io::Result<Self> {
    let mut endpoint = Url::parse(dsn).or_else(|_invalid| invalid_dsn())?;
    let key = endpoint.username().to_string();

    match ::core::primitive::bool::then_some(
      endpoint.scheme() == "https"
        && endpoint.has_host()
        && endpoint.path().trim_end_matches('/') == "/errors/1"
        && endpoint.password().is_none()
        && endpoint.query().is_none()
        && endpoint.fragment().is_none()
        && !key.is_empty()
        && key.bytes().all(|byte| byte.is_ascii_hexdigit()),
      (),
    ) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return invalid_dsn(),
    }

    endpoint.set_username("").or_else(|()| invalid_dsn())?;
    endpoint.set_path("/errors/api/1/envelope/");
    let Ok(hash) = console_core_checksums::sha256::of(dsn.as_bytes());
    let Ok(namespace) = hex::encode(&hash, Case::Lower);

    Ok(Self {
      endpoint,
      key,
      namespace: OutboxName(namespace),
    })
  }

  pub fn endpoint(&self) -> Result<&str, Never> {
    Ok(self.endpoint.as_str())
  }

  pub fn authorization(&self) -> Result<String, Never> {
    Ok(std::format!(
      "X-Sentry-Auth: Sentry sentry_version=7, sentry_key={}",
      self.key
    ))
  }

  pub fn outbox(&self) -> Result<Option<Outbox>, Never> {
    let Ok(state) = Base::State.ours();
    let Ok(runtime) = console_core_places::application_runtime();

    Ok(state.map(|state_base| Outbox {
      state: state_base.join("error-reports").join(&self.namespace),
      runtime:
        runtime.map(|runtime_base| runtime_base.join("error-reports").join(&self.namespace)),
    }))
  }
}

fn invalid_dsn<T>() -> io::Result<T> {
  Err(io::Error::new(
    io::ErrorKind::InvalidInput,
    "ORBIT_ERROR_DSN needs an HTTPS Orbit error DSN",
  ))
}

pub struct Outbox {
  state: PathBuf,
  runtime: Option<PathBuf>,
}

impl Outbox {
  fn queue(&self, envelope: &[u8]) -> io::Result<()> {
    match (save(&self.state, envelope), &self.runtime) {
      (Ok(()), _) => Ok(()),
      (Err(_state_unavailable), Some(runtime)) => save(runtime, envelope),
      (Err(fault), None) => Err(fault),
    }
  }

  pub fn send_pending(&self, mut send: impl FnMut(&Path) -> io::Result<()>) -> io::Result<()> {
    private_directory(&self.state)?;
    let lock = File::open(&self.state)?;

    match flock(&lock, FlockOperation::NonBlockingLockExclusive) {
      Ok(()) => {}
      Err(rustix::io::Errno::WOULDBLOCK) => return Ok(()),
      Err(fault) => return Err(io::Error::from(fault)),
    }

    match &self.runtime {
      Some(runtime) => {
        let Ok(queued) = pending(runtime);

        for report in queued {
          let envelope = read_bytes(&report)?;

          match save(&self.state, &envelope) {
            Ok(()) => fs::remove_file(report)?,
            Err(fault) => match ::core::primitive::bool::then_some(
              fault.kind() == io::ErrorKind::StorageFull,
              (),
            ) {
              ::core::option::Option::Some(()) => break,
              ::core::option::Option::None => return Err(fault),
            },
          }
        }
      }
      None => {}
    }

    let Ok(queued) = pending(&self.state);

    for report in queued.take(16) {
      send(&report)?;
      fs::remove_file(report)?;
    }

    Ok(())
  }
}

fn private_directory(directory: &Path) -> io::Result<()> {
  DirBuilder::new()
    .mode(0o700)
    .recursive(true)
    .create(directory)?;
  let metadata = fs::symlink_metadata(directory)?;

  match ::core::primitive::bool::then_some(
    metadata.is_dir()
      && metadata.mode() & 0o077 == 0
      && metadata.uid() == rustix::process::geteuid().as_raw(),
    (),
  ) {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(io::Error::new(
      io::ErrorKind::PermissionDenied,
      "the Orbit report directory must be private to its owner",
    )),
  }
}

fn slots(directory: &Path) -> Result<impl Iterator<Item = PathBuf> + '_, Never> {
  Ok((0..SLOTS).map(|slot| directory.join(OutboxName(std::format!("{slot:03}.envelope")))))
}

fn pending(directory: &Path) -> Result<impl Iterator<Item = PathBuf> + '_, Never> {
  let Ok(candidates) = slots(directory);

  Ok(candidates.filter(|at| match fs::symlink_metadata(at) {
    Ok(metadata) => metadata.is_file(),
    Err(_unavailable) => false,
  }))
}

fn save(directory: &Path, envelope: &[u8]) -> io::Result<()> {
  private_directory(directory)?;
  let Ok(candidates) = slots(directory);

  for slot in candidates {
    match fs::symlink_metadata(&slot) {
      Ok(_occupied) => continue,
      Err(fault) => {
        match ::core::primitive::bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => return Err(fault),
        }
      }
    }

    match whole_without_overwriting(&slot, envelope) {
      Ok(()) => return Ok(()),
      Err(Unwritten::Moving(at, fault)) => {
        match ::core::primitive::bool::then_some(fault.kind() == io::ErrorKind::AlreadyExists, ())
        {
          ::core::option::Option::Some(()) => continue,
          ::core::option::Option::None => {
            return Err(io::Error::other(Unwritten::Moving(at, fault)));
          }
        }
      }
      Err(fault) => return Err(io::Error::other(fault)),
    }
  }

  Err(io::Error::new(
    io::ErrorKind::StorageFull,
    "the Orbit report queue is full",
  ))
}

#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "the process-wide reporting configuration is read once and then immutable"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  expect(
    explicit044_no_ambient_value,
    reason = "the host opts in through the configuration boundary and all library diagnostics share that fixed destination"
  )
)]
static OUTBOX: OnceLock<Option<Outbox>> = OnceLock::new();

#[derive(Clone, Copy)]
enum ReportLevel {
  Error,
  Fatal,
}

pub(super) fn keep_error(module: &str, message: fmt::Arguments<'_>) -> Result<(), Never> {
  keep(ReportLevel::Error, module, message)
}

pub fn keep_crash(unit: &str, message: fmt::Arguments<'_>) -> Result<(), Never> {
  keep(ReportLevel::Fatal, unit, message)
}

fn keep(level: ReportLevel, module: &str, message: fmt::Arguments<'_>) -> Result<(), Never> {
  let outbox = OUTBOX.get_or_init(|| match Destination::configured() {
    Ok(Some(destination)) => {
      let Ok(outbox) = destination.outbox();

      outbox
    }
    Ok(None) => None,
    Err(fault) => {
      crate::warn!("{fault}");
      None
    }
  });

  match outbox {
    Some(queue) => {
      let kept =
        envelope(level, module, message).and_then(|report| queue.queue(report.as_bytes()));

      match kept {
        Ok(()) => {}
        Err(fault) => crate::warn!("could not queue an Orbit error report: {fault}"),
      }
    }
    None => {}
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit077_no_ambient_randomness",
  expect(
    explicit077_no_ambient_randomness,
    reason = "the reporting boundary creates a unique id that is preserved across upload retries"
  )
)]
fn envelope(level: ReportLevel, module: &str, message: fmt::Arguments<'_>) -> io::Result<String> {
  let mut random = [MaybeUninit::<u8>::uninit(); 16];
  let (id, missing) = getrandom(&mut random, GetRandomFlags::empty())?;

  match ::core::primitive::bool::then_some(missing.is_empty(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      return Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "short report id",
      ));
    }
  }

  let Ok(event_id) = hex::encode(id, Case::Lower);
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "Iterator::take uses a native count for the fixed report character limit"
    )
  )]
  let formatted: String = message
    .to_string()
    .chars()
    .take(usize::from(MESSAGE_CHARS))
    .collect();
  let level_name = match level {
    ReportLevel::Error => "error",
    ReportLevel::Fatal => "fatal",
  };
  #[cfg_attr(
    all(not(test), dylint_lib = "explicit039_no_reading_the_clock"),
    expect(
      explicit039_no_reading_the_clock,
      reason = "this reporting boundary timestamps a report when it is queued for later delivery"
    )
  )]
  let timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
    Ok(time) => Some(time.as_secs()),
    Err(_clock_before_epoch) => None,
  };
  let event = json!({
    "event_id": event_id,
    "platform": "native",
    "logger": module,
    "level": level_name,
    "timestamp": timestamp,
    "message": { "formatted": formatted },
    "extra": { "component": module, "severity": level_name, "occurred_unix_seconds": timestamp },
  });
  let header = json!({ "event_id": event_id });
  let item = json!({ "type": "event" });

  Ok(std::format!("{header}\n{item}\n{event}\n"))
}

#[cfg(test)]
mod tests {
  use super::{Case, hex};

  #[test]
  fn report_ids_keep_leading_zeroes_and_lowercase_hexadecimal() {
    let Ok(id) = hex::encode(&[0x00, 0x1a, 0xc5, 0xff], Case::Lower);

    assert_eq!(id, "001ac5ff");
  }
}
