use std::fs;
use std::io::{self, ErrorKind, Read};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use console_login_window::stored_pattern::{self, Hash, Matched, StoredPattern};

use super::{Fault, desktop_account, optional, version, write};

const ENROLLED: &str = "update-pattern";
const ATTEMPT: &str = "update-pattern-attempt";
const WAIT_SECONDS: u64 = 5;

pub(super) struct Order {
  pub(super) tag: String,
  pattern: String,
}

impl Order {
  pub(super) fn read() -> Result<Self, Fault> {
    let mut request = String::new();
    io::stdin().take(65).read_to_string(&mut request)?;
    Self::parse(request).ok_or_else(|| "invalid update pattern request".into())
  }

  fn parse(request: String) -> Option<Self> {
    let (tag, pattern) = request.strip_suffix('\n')?.split_once('\n')?;
    version(tag)?;
    if !(4..=9).contains(&pattern.len()) {
      return None;
    }
    let mut seen = 0_u16;
    for dot in pattern.bytes() {
      if !(b'a'..=b'i').contains(&dot) {
        return None;
      }
      let mark = 1 << (dot - b'a');
      if seen & mark != 0 {
        return None;
      }
      seen |= mark;
    }
    Some(Self {
      tag: tag.to_owned(),
      pattern: pattern.to_owned(),
    })
  }
}

pub(super) fn enrolled(releases: &Path) -> Result<bool, Fault> {
  Ok(hash(releases)?.is_some())
}

pub(super) fn enroll(releases: &Path) -> Result<(), Fault> {
  let account = desktop_account()?;
  let home = account
    .split(':')
    .nth(5)
    .map(Path::new)
    .filter(|path| path.is_absolute())
    .ok_or("the desktop account has no home directory")?;
  let StoredPattern::Hash(hash) = stored_pattern::stored(home)? else {
    return Err("set a login pattern before enrolling it for Console updates".into());
  };
  validate(&hash.0)?;
  write(&releases.join(ENROLLED), &hash.0)?;
  println!("The login pattern is enrolled for Console updates.");
  Ok(())
}

pub(super) fn forget(releases: &Path) -> Result<(), Fault> {
  match fs::remove_file(releases.join(ENROLLED)) {
    Ok(()) => Ok(()),
    Err(fault) if fault.kind() == ErrorKind::NotFound => Ok(()),
    Err(fault) => Err(fault.into()),
  }
}

pub(super) fn verify(releases: &Path, order: &Order) -> Result<(), Fault> {
  let hash = hash(releases)?.ok_or("no pattern is enrolled for Console updates")?;
  let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
  let attempt = releases.join(ATTEMPT);
  if let Some(last) = optional(&attempt)? {
    if now < last.parse::<u64>()?.saturating_add(WAIT_SECONDS) {
      return Err("wait a few seconds before trying the update pattern again".into());
    }
  }
  write(&attempt, &now.to_string())?;
  let Ok(matched) = stored_pattern::matches(&order.pattern, &hash);
  match matched {
    Matched::Yes => Ok(()),
    Matched::No => Err("the update pattern did not match".into()),
  }
}

fn hash(releases: &Path) -> Result<Option<Hash>, Fault> {
  optional(&releases.join(ENROLLED))?
    .map(|hash| {
      validate(&hash)?;
      Ok(Hash(hash))
    })
    .transpose()
}

fn validate(hash: &str) -> Result<(), Fault> {
  let parts: Vec<_> = hash.split('$').collect();
  if let ["", "y", settings, salt, checksum] = parts.as_slice() {
    if (1..=8).contains(&settings.len())
      && salt.len() == 22
      && checksum.len() == 43
      && [settings, salt, checksum].iter().all(|part| {
        part
          .bytes()
          .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'/'))
      })
    {
      return Ok(());
    }
  }
  Err("the enrolled update pattern does not contain a supported yescrypt hash".into())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn only_the_release_tag_and_distinct_dots_are_accepted() {
    assert!(Order::parse("legion-go-v0.1.2\naidc\n".to_owned()).is_some());
    for request in [
      "legion-go-v0.1.2\naabca\n",
      "legion-go-v0.1.2\nabcj\n",
      "legion-go-v0.1.2\nabc\n",
      "legion-go-v0.1.2\nabcd\nignored\n",
      "--rollback\nabcd\n",
    ] {
      assert!(Order::parse(request.to_owned()).is_none());
    }
  }

  #[test]
  fn enrollment_accepts_the_login_windows_hash_format() -> Result<(), Fault> {
    let hash = stored_pattern::hashed("abcd")?;
    validate(&hash.0)
  }
}
