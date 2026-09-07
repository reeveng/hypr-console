//! Where a picture's source comes from, and how it is known to be the one.
//!
//! The artist's loops are not kept in this repository. They are somebody else's
//! work, they are twenty megabytes each, and this repository is source and
//! nothing else: the device compiles the programs and it presses the pictures,
//! from a list of addresses and the checksum each one had when it was written
//! down.
//!
//! The checksum is not there to catch a bad download, which curl already
//! refuses. It is there because these are fetched from a site that mirrors
//! somebody else's work, and a picture that quietly becomes a different picture
//! is worse than one that fails to arrive. A mismatch stops that one picture
//! and says so, and every other picture is pressed as usual.

use std::path::{Path, PathBuf};

use console_core_external_programs::Program;
use console_core_never::Never;
use sha2::{Digest, Sha256};

#[derive(Debug, PartialEq, Eq)]
pub enum Got {
    Held,
    Fetched,
    Changed { wanted: String, found: String },
}

pub fn kept() -> Result<PathBuf, Never> {
    let said = crate::place::said("XDG_CACHE_HOME")?;

    let cache = match said {
        Some(cache) => PathBuf::from(cache),
        None => {
            let said = crate::place::said("HOME")?;

            let home = said.unwrap_or_else(|| "/tmp".to_string());

            Path::new(&home).join(".cache")
        }
    };

    Ok(cache.join("console/sky"))
}

pub fn checksum(at: &Path) -> Result<String, String> {
    let held = std::fs::read(at)
        .map_err(|fault| format!("{} could not be read: {fault}", at.display()))?;
    Ok(format!("{:x}", Sha256::digest(&held)))
}

pub fn is_the_one(at: &Path, wanted: &str) -> Result<bool, String> {
    match wanted.is_empty() {
        true => Ok(true),
        false => {
            let found = checksum(at)?;

            Ok(found == wanted.trim().to_lowercase())
        }
    }
}

pub fn get(from: &str, wanted: &str, into: &Path) -> Result<Got, String> {
    match into.is_file() {
        true => {
            let held = is_the_one(into, wanted)?;

            return match held {
                true => Ok(Got::Held),
                false => {
                    let found = checksum(into)?;

                    Ok(Got::Changed { wanted: wanted.to_string(), found })
                }
            };
        }
        false => {},
    }

    match into.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{} could not be made: {fault}", holding.display()))?,
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
        .map_err(|fault| format!("curl would not run: {fault}"))?;

    match done.status.success() {
        true => {},
        false => {
            let _ = std::fs::remove_file(&part);
            return Err(format!(
                "{from} would not come: {}",
                String::from_utf8_lossy(&done.stderr).trim()
            ));
        }
    }

    let matched = is_the_one(&part, wanted)?;

    match matched {
        true => {},
        false => {
            let found = checksum(&part)?;
            let _ = std::fs::remove_file(&part);
            return Ok(Got::Changed { wanted: wanted.to_string(), found });
        }
    }

    std::fs::rename(&part, into)
        .map_err(|fault| format!("{} could not be put in place: {fault}", into.display()))?;
    Ok(Got::Fetched)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let at = std::env::temp_dir().join(format!("console-sky-{name}"));
        let _ = std::fs::remove_file(&at);
        at
    }

    #[test]
    fn a_checksum_is_what_sha256sum_would_say() {
        let at = scratch("checksum");
        std::fs::write(&at, b"a picture").expect("a file");
        assert_eq!(
            checksum(&at).expect("a checksum"),
            "92b2fa58028958317e408bd84ecfa70f5ee35b121991dbc232c49d166353708b"
        );
        let _ = std::fs::remove_file(&at);
    }

    #[test]
    fn a_file_with_nothing_written_down_about_it_is_taken_on_trust() {
        let at = scratch("trusted");
        std::fs::write(&at, b"hers").expect("a file");
        assert_eq!(is_the_one(&at, ""), Ok(true));
        let _ = std::fs::remove_file(&at);
    }

    #[test]
    fn a_file_that_is_not_what_was_written_down_is_not_the_one() {
        let at = scratch("changed");
        std::fs::write(&at, b"a different picture").expect("a file");
        assert_eq!(is_the_one(&at, "0".repeat(64).as_str()), Ok(false));
        let _ = std::fs::remove_file(&at);
    }

    #[test]
    fn a_source_already_here_and_right_is_held_rather_than_fetched() {
        let at = scratch("held");
        std::fs::write(&at, b"a picture").expect("a file");
        let got = get(
            "https://example.invalid/never-asked",
            "92b2fa58028958317e408bd84ecfa70f5ee35b121991dbc232c49d166353708b",
            &at,
        );
        assert_eq!(got, Ok(Got::Held));
        let _ = std::fs::remove_file(&at);
    }

    #[test]
    fn a_source_here_that_has_changed_says_so_rather_than_being_used() {
        let at = scratch("swapped");
        std::fs::write(&at, b"a picture").expect("a file");
        let got = get("https://example.invalid/never-asked", &"0".repeat(64), &at);
        assert!(matches!(got, Ok(Got::Changed { .. })), "{got:?}");
        let _ = std::fs::remove_file(&at);
    }
}
