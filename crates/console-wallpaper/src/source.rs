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

use console_core_external_programs::Program;
use console_core_never::Never;
use sha2::{Digest, Sha256};

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
    let held = std::fs::read(at)
        .map_err(|fault| Unpainted::Read(at.to_path_buf(), fault))?;
    Ok(format!("{:x}", Sha256::digest(&held)))
}

pub fn is_the_one(at: &Path, wanted: &str) -> Result<bool, Unpainted> {
    match wanted.is_empty() {
        true => Ok(true),
        false => {
            let found = checksum(at)?;

            Ok(found == wanted.trim().to_lowercase())
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

    match into.is_file() {
        true => {
            let held = is_the_one(into, wanted)?;

            return match held {
                true => Ok(NameRequestResult::Cached),
                false => {
                    let found = checksum(into)?;

                    Ok(NameRequestResult::Replaced { wanted: wanted.to_string(), found })
                }
            };
        }
        false => {},
    }

    match into.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| Unpainted::Holding(holding.to_path_buf(), fault))?,
        None => {},
    }

    let part = into.with_extension("part");
    let Ok(mut asking) = Program::Curl.command();

    let done = asking
        .args(["--silent", "--show-error", "--fail", "--location", "--max-time", "600"])
        .arg("--output")
        .arg(&part)
        .arg(from)
        .output()
        .map_err(Unpainted::NoCurl)?;

    match done.status.success() {
        true => {},
        false => {
            let _ = std::fs::remove_file(&part);
            return Err(Unpainted::Unfetched(
                from.to_string(),
                String::from_utf8_lossy(&done.stderr).trim().to_string(),
            ));
        }
    }

    let matched = is_the_one(&part, wanted)?;

    match matched {
        true => {},
        false => {
            let found = checksum(&part)?;
            let _ = std::fs::remove_file(&part);
            return Ok(NameRequestResult::Replaced { wanted: wanted.to_string(), found });
        }
    }

    std::fs::rename(&part, into)
        .map_err(|fault| Unpainted::Unplaced(into.to_path_buf(), fault))?;
    Ok(NameRequestResult::Fetched)
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
        let trusted = is_the_one(&at, "")?;

        assert!(trusted);

        std::fs::remove_file(&at)?;

        Ok(())
    }

    #[test]
    fn a_file_that_is_not_what_was_written_down_is_not_the_one() -> Result<(), Box<dyn Error>> {
        let at = scratch("changed", b"a different picture")?;
        let the_one = is_the_one(&at, "0".repeat(64).as_str())?;

        assert!(!the_one);

        std::fs::remove_file(&at)?;

        Ok(())
    }

    #[test]
    fn a_source_already_here_and_right_is_held_rather_than_fetched() -> Result<(), Box<dyn Error>> {
        let at = scratch("held", b"a picture")?;
        let got = get(Source { from: "https://example.invalid/never-asked", wanted: A_PICTURE }, &at)?;

        assert_eq!(got, NameRequestResult::Cached);

        std::fs::remove_file(&at)?;

        Ok(())
    }

    #[test]
    fn a_source_here_that_has_changed_says_so_rather_than_being_used() -> Result<(), Box<dyn Error>> {
        let at = scratch("swapped", b"a picture")?;
        let wanted = "0".repeat(64);
        let source = Source { from: "https://example.invalid/never-asked", wanted: &wanted };
        let got = get(source, &at);

        assert!(matches!(got, Ok(NameRequestResult::Replaced { .. })), "{got:?}");

        std::fs::remove_file(&at)?;

        Ok(())
    }
}
