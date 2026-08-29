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
use std::process::Command;

use sha2::{Digest, Sha256};

/// What was made of a source that was asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum Got {
    /// It was already here, and it is the right one.
    Held,
    /// It was fetched.
    Fetched,
    /// It is here and it is not what was written down.
    Changed { wanted: String, found: String },
}

/// Where sources are kept between one press and the next.
///
/// Under the cache rather than beside the pictures, because they are not needed
/// after a press and anything under a cache is a thing a machine may delete.
pub fn kept() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new(&home).join(".cache"))
        .join("console/sky")
}

/// What a file's checksum is, written the way `sha256sum` writes it.
pub fn checksum(at: &Path) -> Result<String, String> {
    let held = std::fs::read(at)
        .map_err(|fault| format!("{} could not be read: {fault}", at.display()))?;
    Ok(format!("{:x}", Sha256::digest(&held)))
}

/// Whether a file is the one that was written down.
///
/// A picture with no checksum against its name is taken on trust. That is how
/// one of hers is handled: she put the file there, and there is nothing for it
/// to have changed from.
pub fn is_the_one(at: &Path, wanted: &str) -> Result<bool, String> {
    match wanted.is_empty() {
        true => Ok(true),
        false => Ok(checksum(at)? == wanted.trim().to_lowercase()),
    }
}

/// Fetch a source unless it is already here, and say which happened.
pub fn get(from: &str, wanted: &str, into: &Path) -> Result<Got, String> {
    if into.is_file() {
        return match is_the_one(into, wanted)? {
            true => Ok(Got::Held),
            false => Ok(Got::Changed {
                wanted: wanted.to_string(),
                found: checksum(into)?,
            }),
        };
    }
    if let Some(holding) = into.parent() {
        std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{} could not be made: {fault}", holding.display()))?;
    }

    // Written beside the name it wants and moved onto it, so a fetch that is
    // interrupted leaves nothing that looks like a finished source. The device
    // is a handheld and the thing that interrupts a download is somebody
    // closing the lid.
    let part = into.with_extension("part");
    let done = Command::new("curl")
        .args(["--silent", "--show-error", "--fail", "--location", "--max-time", "600"])
        .arg("--output")
        .arg(&part)
        .arg(from)
        .output()
        .map_err(|fault| format!("curl would not run: {fault}"))?;
    if !done.status.success() {
        let _ = std::fs::remove_file(&part);
        return Err(format!(
            "{from} would not come: {}",
            String::from_utf8_lossy(&done.stderr).trim()
        ));
    }

    if !is_the_one(&part, wanted)? {
        let found = checksum(&part)?;
        let _ = std::fs::remove_file(&part);
        return Ok(Got::Changed { wanted: wanted.to_string(), found });
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
        // Worked out by sha256sum, and written down so this cannot agree with
        // itself about a wrong answer.
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

    /// A source already here and right is not fetched again, which is what
    /// makes a second press quick and an apply with no network possible.
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
