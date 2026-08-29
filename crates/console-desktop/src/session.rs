//! Which compositor is which, and waiting for one to arrive.
//!
//! A nested Hyprland picks its own signature and its own socket and says
//! neither, so both are learned by watching for one appearing.

use std::collections::BTreeSet;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::{runtime, stages};

/// How long a compositor is given to come up.
pub const COMING_UP: Duration = Duration::from_secs(15);

/// How long its socket is given to appear.
pub const A_SOCKET: Duration = Duration::from_secs(10);

/// How often anything here looks again.
const BREATH: Duration = Duration::from_millis(100);

/// The Hyprlands running on this machine, by their signature.
pub fn instances() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(runtime().join("hypr")) else { return found };
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.is_dir() && let Some(name) = path.file_name() {
            found.insert(name.to_string_lossy().to_string());
        }
    }
    found
}

/// The Wayland displays this machine is offering right now.
pub fn sockets() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(runtime()) else { return found };
    for path in entries.flatten().map(|entry| entry.path()) {
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        if name.starts_with("wayland-") && !name.ends_with(".lock") {
            found.insert(name);
        }
    }
    found
}

fn until<T>(patience: Duration, mut look: impl FnMut() -> Option<T>) -> Option<T> {
    let by = Instant::now() + patience;
    while Instant::now() < by {
        if let Some(found) = look() {
            return Some(found);
        }
        std::thread::sleep(BREATH);
    }
    None
}

/// The nested compositor's own signature, which is how it is spoken to.
///
/// It picks this itself and does not say what it picked, so what is watched for
/// is a new one appearing with a socket in it. Without it, hyprctl talks to
/// whichever Hyprland this session belongs to, which is the one running the
/// screen you are looking at.
pub fn wait_for_instance(was: &BTreeSet<String>) -> Option<String> {
    until(COMING_UP, || {
        instances()
            .difference(was)
            .find(|name| runtime().join("hypr").join(name).join(".socket.sock").exists())
            .cloned()
    })
}

/// The socket the nested compositor just put down.
///
/// It cannot be asked for by name: --socket is half of the handover pair and
/// Hyprland refuses it alone. So it names itself, and this waits to see which
/// name that was.
pub fn wait_for_socket(was: &BTreeSet<String>) -> Option<String> {
    until(A_SOCKET, || sockets().difference(was).next().cloned())
}

/// The runtime directory a killed compositor does not tidy up itself.
pub fn left_behind(signature: &str) {
    let _ = std::fs::remove_dir_all(runtime().join("hypr").join(signature));
}

/// Stages belonging to sessions that are no longer running.
pub fn abandoned() -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(stages()) else { return Vec::new() };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let Some(pid) = name.strip_prefix("session-") else { return false };
            pid.chars().all(|digit| digit.is_ascii_digit())
                && !PathBuf::from("/proc").join(pid).exists()
        })
        .collect();
    found.sort();
    found
}

/// Runtime directories of compositors that are not there any more.
///
/// A nested Hyprland that is killed rather than asked to leave does not tidy its
/// own, and they had been piling up since the first one. A live one answers
/// hyprctl; that is the whole test, and the one this session belongs to is left
/// alone whatever it says.
pub fn dead_instances() -> Vec<PathBuf> {
    let ours = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default();
    instances()
        .into_iter()
        .filter(|name| *name != ours)
        .filter(|name| {
            !std::process::Command::new("hyprctl")
                .args(["-i", name, "version"])
                .output()
                .is_ok_and(|done| done.status.success())
        })
        .map(|name| runtime().join("hypr").join(name))
        .collect()
}

/// Held while a compositor comes up, so two of them cannot be confused.
///
/// A nested Hyprland picks its own signature and does not say what it picked, so
/// the only way to learn it is to watch for one appearing. Two sessions starting
/// at the same moment would each see two appear and could take the other's.
/// Coming up takes a second or so; this is held for that second, and dropped
/// long before the desktop is used, so sessions still run at once.
pub struct Starting {
    held: Option<File>,
}

impl Starting {
    pub fn now() -> Self {
        let _ = std::fs::create_dir_all(stages());
        let held = File::create(stages().join("starting.lock")).ok();
        if let Some(file) = &held {
            // SAFETY: the descriptor is this file's, and open for the call.
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        }
        Starting { held }
    }
}

impl Drop for Starting {
    fn drop(&mut self) {
        if let Some(file) = &self.held {
            // SAFETY: as above, and this is the handle that took it.
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one this session belongs to is left alone whatever it says.
    #[test]
    fn the_compositor_running_this_screen_is_never_one_of_the_dead() {
        let ours = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default();
        if ours.is_empty() {
            return;
        }
        assert!(!dead_instances().iter().any(|path| path.ends_with(&ours)));
    }

    #[test]
    fn a_stage_is_abandoned_only_when_the_session_it_names_has_ended() {
        let ours = crate::stage();
        assert!(!abandoned().contains(&ours), "this session's own stage is not abandoned");
    }
}
