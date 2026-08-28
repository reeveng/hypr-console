//! Putting one file where the manifest says it goes.

use std::path::{Path, PathBuf};

/// The user whose desktop this is.
pub const USER: &str = "player";

/// How a file on the machine stands against the source it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The machine has it and it is what the source says.
    Ok,
    /// The machine has it and it is not what the source says.
    Differs,
    /// The source has it and the machine does not.
    Missing,
    /// The manifest names it and nothing holds its content.
    Unsourced,
}

impl State {
    pub fn name(self) -> &'static str {
        match self {
            State::Ok => "ok",
            State::Differs => "differs",
            State::Missing => "missing",
            State::Unsourced => "unsourced",
        }
    }

    pub fn settled(self) -> bool {
        self == State::Ok
    }
}

/// Where under the source tree the content of a live path is kept.
///
/// `/usr/local/bin/launcher` is `files/usr/local/bin/launcher`. There is
/// nothing to keep in step, because the one path is the other.
pub fn source_of(source: &Path, live: &str) -> PathBuf {
    source.join(live.trim_start_matches('/'))
}

/// One of missing, differs, ok, or unsourced.
pub fn state(source: &Path, live: &str) -> State {
    let (from, to) = (source_of(source, live), Path::new(live));
    match (std::fs::read(&from), std::fs::read(to)) {
        (Err(_), _) => State::Unsourced,
        (Ok(_), Err(_)) => State::Missing,
        (Ok(held), Ok(there)) if held == there => State::Ok,
        (Ok(_), Ok(_)) => State::Differs,
    }
}

/// Files under a user's home belong to that user. Everything else is root's.
pub fn owner_of(live: &str) -> &'static str {
    match live.starts_with(&format!("/home/{USER}/")) {
        true => USER,
        false => "root",
    }
}

/// Whether a file is meant to be run.
///
/// A script says so with a shebang and a compiled program with its own magic
/// number, and anything kept in a bin directory is there to be run whatever it
/// is made of. Reading only the first two bytes got this wrong for a compiled
/// program, which was then installed unreadable to the kernel and refused to
/// start with nothing but "permission denied" to say why.
pub fn mode_of(live: &str, head: &[u8]) -> u32 {
    match live {
        path if path.contains("/bin/") || path.contains("/sbin/") => 0o755,
        _ => match head {
            [b'#', b'!', ..] => 0o755,
            [0x7f, b'E', b'L', b'F', ..] => 0o755,
            _ => 0o644,
        },
    }
}

/// The first bytes of a file, for deciding whether it is meant to be run.
pub fn head_of(path: &Path) -> Vec<u8> {
    std::fs::read(path).map(|held| held.into_iter().take(4).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_path_is_the_live_path_under_the_tree() {
        let source = Path::new("/etc/legion/files");
        assert_eq!(
            source_of(source, "/usr/local/bin/launcher"),
            Path::new("/etc/legion/files/usr/local/bin/launcher")
        );
    }

    #[test]
    fn a_file_in_a_home_belongs_to_whoever_lives_there() {
        assert_eq!(owner_of("/home/player/.config/hypr/hyprland.lua"), USER);
        assert_eq!(owner_of("/etc/systemd/user/legion.target"), "root");
        // Not any home, and not a name that merely starts the same way.
        assert_eq!(owner_of("/home/playera/.bashrc"), "root");
        assert_eq!(owner_of("/home/someone/.bashrc"), "root");
    }

    #[test]
    fn anything_in_a_bin_directory_is_meant_to_be_run() {
        assert_eq!(mode_of("/usr/local/bin/legion", b"any"), 0o755);
        assert_eq!(mode_of("/usr/sbin/thing", b""), 0o755);
    }

    #[test]
    fn a_script_and_a_compiled_program_are_both_meant_to_be_run() {
        assert_eq!(mode_of("/etc/thing", b"#!/bin/sh"), 0o755);
        assert_eq!(mode_of("/etc/thing", b"\x7fELF\x02"), 0o755);
    }

    #[test]
    fn a_compiled_program_is_read_past_its_first_two_bytes() {
        // Reading only two bytes installed a binary unreadable to the kernel,
        // which then refused to start with nothing but "permission denied".
        assert_eq!(mode_of("/etc/thing", b"\x7fELF"), 0o755);
        assert_eq!(mode_of("/etc/thing", b"\x7fEL"), 0o644);
    }

    #[test]
    fn everything_else_is_only_read() {
        assert_eq!(mode_of("/etc/systemd/user/legion.target", b"[Uni"), 0o644);
        assert_eq!(mode_of("/home/player/.config/kdeglobals", b"[Col"), 0o644);
    }
}
