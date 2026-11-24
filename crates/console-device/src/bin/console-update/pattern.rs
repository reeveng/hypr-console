use std::fs;
use std::io::ErrorKind;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use console_login_window::stored_pattern::{self, Hash, Matched, StoredPattern};
use console_operating_system_linux::program_lifetime::TimedReadPipe;

use super::{UpdateError, desktop_account, optional, undo_target, version, write};

const ENROLLED: &str = "update-pattern";
const ATTEMPT: &str = "update-pattern-attempt";
const WAIT_SECONDS: u64 = 5;

pub(super) struct Order {
  pub(super) action: Action,
  pattern: String,
}

pub(super) enum Action {
  Install(String),
  Undo(String),
}

const INVALID: &str = "invalid update pattern request";

impl Order {
  pub(super) fn read(input: OwnedFd, clock: impl Fn() -> Instant) -> Result<Self, UpdateError> {
    const WAIT_FOR_REQUEST: Duration = Duration::from_secs(5);
    let deadline = clock().checked_add(WAIT_FOR_REQUEST).ok_or(INVALID)?;
    let pipe = TimedReadPipe::new(input)?;
    let mut bytes = [0_u8; 128];
    let request_bytes = pipe.read_until(&mut bytes, deadline, clock)?;
    let request = std::str::from_utf8(request_bytes).map_err(|_| INVALID)?;
    Self::parse(request)
  }

  fn parse(request: &str) -> Result<Self, UpdateError> {
    let submitted = request.strip_suffix('\n').ok_or(INVALID)?;
    let (requested, pattern) = submitted.split_once('\n').ok_or(INVALID)?;
    let action = match requested.starts_with("undo:").then_some(()) {
      Some(()) => {
        let _slot = undo_target(requested).map_err(|_| INVALID)?;
        Action::Undo(requested.to_owned())
      }
      None => {
        let _version = version(requested).map_err(|_| INVALID)?;
        Action::Install(requested.to_owned())
      }
    };

    match pattern.len() {
      4..=9 => {}
      _ => return Err(UpdateError::from(INVALID)),
    }

    let mut seen = 0_u16;

    for dot in pattern.bytes() {
      match dot {
        b'a'..=b'i' => {}
        _ => return Err(UpdateError::from(INVALID)),
      }

      let shift = u32::from(dot.wrapping_sub(b'a'));
      let mark = 1_u16.wrapping_shl(shift);

      match bool::then_some(seen & mark == 0, ()) {
        Some(()) => seen |= mark,
        None => return Err(UpdateError::from(INVALID)),
      }
    }

    Ok(Self {
      action,
      pattern: pattern.to_owned(),
    })
  }
}

pub(super) enum Enrollment {
  Present,
  Absent,
}

pub(super) fn enrolled(releases: &Path) -> Result<Enrollment, UpdateError> {
  let enrolled = hash(releases)?;

  Ok(match enrolled {
    Some(_) => Enrollment::Present,
    None => Enrollment::Absent,
  })
}

pub(super) fn enroll(releases: &Path) -> Result<(), UpdateError> {
  let account = desktop_account()?;
  let home = account
    .split(':')
    .nth(5)
    .map(Path::new)
    .filter(|path| path.is_absolute())
    .ok_or("the desktop account has no home directory")?;

  let stored = stored_pattern::stored(home)?;
  let hash = match stored {
    StoredPattern::Hash(hash) => hash,
    StoredPattern::Absent => {
      return Err(UpdateError::from(
        "set a login pattern before enrolling it for Console updates",
      ));
    }
  };

  validate(&hash.0)?;
  write(&releases.join(ENROLLED), &hash.0)?;
  println!("The login pattern is enrolled for Console updates.");
  Ok(())
}

pub(super) fn forget(releases: &Path) -> Result<(), UpdateError> {
  match fs::remove_file(releases.join(ENROLLED)) {
    Ok(()) => Ok(()),
    Err(fault) => match bool::then_some(fault.kind() == ErrorKind::NotFound, ()) {
      Some(()) => Ok(()),
      None => Err(UpdateError::from(fault)),
    },
  }
}

pub(super) fn verify(releases: &Path, order: &Order) -> Result<(), UpdateError> {
  let enrolled = hash(releases)?;
  let hash = enrolled.ok_or("no pattern is enrolled for Console updates")?;
  let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?;
  let now = elapsed.as_secs();
  let attempt = releases.join(ATTEMPT);

  let last_attempt = optional(&attempt)?;

  match last_attempt {
    Some(last) => {
      let seconds = last.parse::<u64>()?;

      match bool::then_some(now < seconds.saturating_add(WAIT_SECONDS), ()) {
        Some(()) => {
          return Err(UpdateError::from(
            "wait a few seconds before trying the update pattern again",
          ));
        }
        None => {}
      }
    }
    None => {}
  }

  write(&attempt, &now.to_string())?;
  let Ok(matched) = stored_pattern::matches(&order.pattern, &hash);

  match matched {
    Matched::Yes => Ok(()),
    Matched::No => Err(UpdateError::from("the update pattern did not match")),
  }
}

fn hash(releases: &Path) -> Result<Option<Hash>, UpdateError> {
  let record = optional(&releases.join(ENROLLED))?;
  record
    .map(|hash| {
      validate(&hash)?;
      Ok(Hash(hash))
    })
    .transpose()
}

fn validate(hash: &str) -> Result<(), UpdateError> {
  let parts: Vec<_> = hash.split('$').collect();

  match parts.as_slice() {
    ["", "y", settings, salt, checksum] => {
      let valid = (1..=8).contains(&settings.len())
        && salt.len() == 22
        && checksum.len() == 43
        && [settings, salt, checksum].iter().all(|part| {
          part
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'/'))
        });

      match bool::then_some(valid, ()) {
        Some(()) => return Ok(()),
        None => {}
      }
    }
    _ => {}
  }

  Err(UpdateError::from(
    "the enrolled update pattern does not contain a supported yescrypt hash",
  ))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn only_the_release_tag_and_distinct_dots_are_accepted() {
    assert!(matches!(Order::parse("legion-go-v0.1.2\naidc\n"), Ok(_)));

    for request in [
      "legion-go-v0.1.2\naabca\n",
      "legion-go-v0.1.2\nabcj\n",
      "legion-go-v0.1.2\nabc\n",
      "legion-go-v0.1.2\nabcd\nignored\n",
      "--rollback\nabcd\n",
    ] {
      assert!(matches!(Order::parse(request), Err(fault) if fault.to_string() == INVALID));
    }
  }

  #[test]
  fn undo_pattern_approval_names_a_slot_and_revision() {
    let target = format!("undo:release-101-234:{}", "a".repeat(40));
    let order = Order::parse(&format!("{target}\naidc\n"));
    assert!(matches!(order, Ok(Order { action: Action::Undo(saved), .. }) if saved == target));
    assert!(matches!(Order::parse("undo:release-101-234:wrong\naidc\n"), Err(fault) if fault.to_string() == INVALID));
  }

  #[test]
  fn enrollment_accepts_the_login_windows_hash_format() -> Result<(), UpdateError> {
    let hash = stored_pattern::hashed("abcd")?;
    validate(&hash.0)
  }
}
