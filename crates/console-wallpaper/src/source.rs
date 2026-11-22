//! Where a picture's source comes from, and how it is known to be the one.
//!
//! The artist's loops are not kept in this repository. They are someone else's
//! work, they are twenty megabytes each, and this repository is source and
//! nothing else: the device compiles the programs and it renders the pictures,
//! from a list of addresses and the checksum each one had when it was written
//! down.
//!
//! The checksum is not there to catch a bad download, which curl already
//! refuses. It is there because these are fetched from a site that mirrors
//! someone else's work, and a picture that quietly becomes a different picture
//! is worse than one that fails to arrive. A mismatch stops that one picture
//! and says so, and every other picture is rendered as usual.

use std::path::{Path, PathBuf};

use console_core_checksums::sha256;
use console_core_external_programs::Program;
use console_core_never::Never;

use crate::Unpainted;

#[derive(Debug, PartialEq, Eq)]
pub enum NameRequestResult {
  Cached,
  Fetched,
  Replaced { wanted: String, found: String },
}

pub fn cache_path() -> Result<Option<PathBuf>, Never> {
  let ours = console_core_places::Base::Cache.ours()?;

  Ok(ours.map(|at| at.join("sky")))
}

pub fn checksum(at: &Path) -> Result<String, Unpainted> {
  let held = std::fs::read(at).map_err(|fault| Unpainted::Read(at.to_path_buf(), fault))?;
  let Ok(digest) = sha256::of(&held);

  Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[derive(Debug, PartialEq, Eq)]
pub enum ChecksumStatus {
  Unspecified,
  Matches,
  Differs { found: String },
}

pub fn verify_checksum(at: &Path, wanted: &str) -> Result<ChecksumStatus, Unpainted> {
  match wanted {
    "" => Ok(ChecksumStatus::Unspecified),
    _ => {
      let found = checksum(at)?;

      Ok(match found.cmp(&wanted.trim().to_lowercase()) {
        std::cmp::Ordering::Equal => ChecksumStatus::Matches,
        std::cmp::Ordering::Less | std::cmp::Ordering::Greater => {
          ChecksumStatus::Differs { found }
        }
      })
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source<'a> {
  pub from: &'a str,
  pub wanted: &'a str,
}

pub fn get(source: Source<'_>, into: &Path) -> Result<NameRequestResult, Unpainted> {
  let Source { from, wanted } = source;

  let Ok(held) = crate::place::file_status(into);

  match held {
    crate::place::FileStatus::File => {
      let status = verify_checksum(into, wanted)?;
      return match status {
        ChecksumStatus::Unspecified | ChecksumStatus::Matches => Ok(NameRequestResult::Cached),
        ChecksumStatus::Differs { found } => Ok(NameRequestResult::Replaced {
          wanted: wanted.to_string(),
          found,
        }),
      };
    }
    crate::place::FileStatus::Other => {}
  }

  match into.parent() {
    Some(holding) => std::fs::create_dir_all(holding)
      .map_err(|fault| Unpainted::Holding(holding.to_path_buf(), fault))?,
    None => {}
  }

  let Ok(part) = console_core_atomic_writes::beside(into);
  let Ok(mut asking) = Program::Curl.command();

  let done = asking
    .args([
      "--silent",
      "--show-error",
      "--fail",
      "--location",
      "--max-time",
      "600",
    ])
    .arg("--output")
    .arg(&part)
    .arg(from)
    .output()
    .map_err(Unpainted::NoCurl)?;

  match done.status.code() {
    Some(0) => {}
    ::core::option::Option::None | ::core::option::Option::Some(_) => {
      let Ok(()) = discard(&part);
      let Ok(diagnostic) = crate::diagnostic(&done.stderr);
      return Err(Unpainted::Unfetched(from.to_string(), diagnostic));
    }
  }

  let checked = match verify_checksum(&part, wanted) {
    Ok(checked) => checked,
    Err(fault) => {
      let Ok(()) = discard(&part);
      return Err(fault);
    }
  };

  match checked {
    ChecksumStatus::Unspecified | ChecksumStatus::Matches => {}
    ChecksumStatus::Differs { found } => {
      let Ok(()) = discard(&part);
      return Ok(NameRequestResult::Replaced {
        wanted: wanted.to_string(),
        found,
      });
    }
  }

  match std::fs::rename(&part, into) {
    Ok(()) => Ok(NameRequestResult::Fetched),
    Err(fault) => {
      let Ok(()) = discard(&part);
      Err(Unpainted::Unplaced(into.to_path_buf(), fault))
    }
  }
}

fn discard(path: &Path) -> Result<(), Never> {
  match std::fs::remove_file(path) {
    Ok(()) => {}
    Err(fault) => match (fault.kind() == std::io::ErrorKind::NotFound).then_some(()) {
      Some(()) => {}
      None => console_core_logging::warn!("{} could not be cleaned up: {fault}", path.display()),
    },
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;

  const A_PICTURE: &str = "92b2fa58028958317e408bd84ecfa70f5ee35b121991dbc232c49d166353708b";

  fn scratch(name: &str, holding: &[u8]) -> Result<PathBuf, Box<dyn Error>> {
    let folder = console_core_temporary_directories::fresh(&format!("wallpaper-{name}"))?;
    let at = folder.join("picture");

    console_core_atomic_writes::whole(&at, holding)?;

    Ok(at)
  }

  #[test]
  fn a_checksum_is_what_sha256sum_would_say() -> Result<(), Box<dyn Error>> {
    let at = scratch("checksum", b"a picture")?;
    let sum = checksum(&at)?;

    assert_eq!(sum, A_PICTURE);

    std::fs::remove_file(&at)?;

    Ok(())
  }

  #[test]
  fn a_file_with_nothing_written_down_about_it_is_taken_on_trust() -> Result<(), Box<dyn Error>> {
    let at = scratch("trusted", b"hers")?;
    let trusted = verify_checksum(&at, "")?;

    assert_eq!(trusted, ChecksumStatus::Unspecified);

    std::fs::remove_file(&at)?;

    Ok(())
  }

  #[test]
  fn a_file_that_is_not_what_was_written_down_is_not_the_one() -> Result<(), Box<dyn Error>> {
    let at = scratch("changed", b"a different picture")?;
    let the_one = verify_checksum(&at, "0".repeat(64).as_str())?;

    let found = checksum(&at)?;
    assert_eq!(the_one, ChecksumStatus::Differs { found });

    std::fs::remove_file(&at)?;

    Ok(())
  }

  #[test]
  fn a_source_already_here_and_right_is_held_rather_than_fetched() -> Result<(), Box<dyn Error>> {
    let at = scratch("held", b"a picture")?;
    let got = get(
      Source {
        from: "https://example.invalid/never-asked",
        wanted: A_PICTURE,
      },
      &at,
    )?;

    assert_eq!(got, NameRequestResult::Cached);

    std::fs::remove_file(&at)?;

    Ok(())
  }

  #[test]
  fn a_source_here_that_has_changed_says_so_rather_than_being_used() -> Result<(), Box<dyn Error>>
  {
    let at = scratch("swapped", b"a picture")?;
    let wanted = "0".repeat(64);
    let source = Source {
      from: "https://example.invalid/never-asked",
      wanted: &wanted,
    };
    let got = get(source, &at);

    assert!(
      matches!(got, Ok(NameRequestResult::Replaced { .. })),
      "{got:?}"
    );

    std::fs::remove_file(&at)?;

    Ok(())
  }
}
