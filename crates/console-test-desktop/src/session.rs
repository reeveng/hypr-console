//! Which compositor is which, and waiting for one to arrive.
//!
//! A nested Hyprland picks its own signature and its own socket and says
//! neither, so both are learned by watching for one appearing. `Starting` is
//! what makes that sound: one session comes up at a time, so the one new name
//! in the runtime directory is this one's.
//!
//! What is not sound is taking every name that begins with `wayland-` for a
//! display. A session started a moment ago goes on filling that directory after
//! the lock is let go -- the wallpaper daemon opens `wayland-2-awww-daemon.sock`
//! beside the display it draws on -- so the new name arriving while this one
//! waits can belong to the session before it. Then everything after it is asked
//! of somebody else's compositor: the screen it makes is not the screen it looks
//! at, and what it says is that the screen never appeared. A display socket is
//! `wayland-` and a number and nothing else, which is what `named` holds a name
//! to.

use std::collections::BTreeSet;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_waiting::Patience;

use crate::{runtime, stages};

pub const COMING_UP: Duration = Duration::from_secs(15);

pub const A_SOCKET: Duration = Duration::from_secs(10);

const BREATH: Duration = Duration::from_millis(100);

pub fn instances() -> Result<BTreeSet<String>, Never> {
    let mut found = BTreeSet::new();
    let Ok(runtime) = runtime();

    let entries = match std::fs::read_dir(runtime.join("hypr")) {
        Ok(entries) => entries,
        Err(_fault) => return Ok(found),
    };

    for path in entries.flatten().map(|entry| entry.path()) {
        match path.is_dir() {
            true => match path.file_name() {
                Some(name) => {
                    found.insert(name.to_string_lossy().to_string());
                }
                None => {},
            },
            false => {},
        }
    }

    Ok(found)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Names {
    ADisplay,
    SomethingElse,
}

pub fn named(name: &str) -> Result<Names, Never> {
    let rest = match name.strip_prefix("wayland-") {
        Some(rest) => rest,
        None => return Ok(Names::SomethingElse),
    };

    Ok(match !rest.is_empty() && rest.chars().all(|said| said.is_ascii_digit()) {
        true => Names::ADisplay,
        false => Names::SomethingElse,
    })
}

pub fn sockets() -> Result<BTreeSet<String>, Never> {
    let mut found = BTreeSet::new();
    let Ok(runtime) = runtime();

    let entries = match std::fs::read_dir(runtime) {
        Ok(entries) => entries,
        Err(_fault) => return Ok(found),
    };

    for path in entries.flatten().map(|entry| entry.path()) {
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

        let Ok(names) = named(&name);

        match names {
            Names::ADisplay => {
                found.insert(name);
            }
            Names::SomethingElse => {},
        }
    }

    Ok(found)
}

fn until<T>(
    patience: Duration,
    mut look: impl FnMut() -> Option<T>,
) -> Result<Option<T>, Never> {
    let Ok(patience) = Patience::asking_every(patience, BREATH);

    console_waiting::found(patience, || Ok(look()))
}

pub fn wait_for_instance(was: &BTreeSet<String>) -> Result<Option<String>, Never> {
    until(COMING_UP, || {
        let Ok(instances) = instances();
        let Ok(runtime) = runtime();

        instances
            .difference(was)
            .find(|name| runtime.join("hypr").join(name).join(".socket.sock").exists())
            .cloned()
    })
}

pub fn wait_for_socket(was: &BTreeSet<String>) -> Result<Option<String>, Never> {
    until(A_SOCKET, || {
        let Ok(sockets) = sockets();

        sockets.difference(was).next().cloned()
    })
}

pub const A_LINE: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrote {
    Something,
    Nothing,
}

pub fn wait_for_written(at: &Path, patience: Duration) -> Result<Wrote, Never> {
    let Ok(found) = until(patience, || {
        let read = match std::fs::read(at) {
            Ok(read) => read,
            Err(_fault) => return None,
        };

        match read.contains(&b'\n') {
            true => Some(()),
            false => None,
        }
    });

    Ok(match found {
        Some(()) => Wrote::Something,
        None => Wrote::Nothing,
    })
}

pub fn left_behind(signature: &str) -> Result<(), Never> {
    let Ok(runtime) = runtime();
    let _ = std::fs::remove_dir_all(runtime.join("hypr").join(signature));

    Ok(())
}

pub fn abandoned() -> Result<Vec<PathBuf>, Never> {
    let Ok(stages) = stages();

    let entries = match std::fs::read_dir(stages) {
        Ok(entries) => entries,
        Err(_fault) => return Ok(Vec::new()),
    };

    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

            let pid = match name.strip_prefix("session-") {
                Some(pid) => pid,
                None => return false,
            };

            pid.chars().all(|digit| digit.is_ascii_digit())
                && !PathBuf::from("/proc").join(pid).exists()
        })
        .collect();
    found.sort();

    Ok(found)
}

pub fn dead_instances() -> Result<Vec<PathBuf>, Never> {
    let Ok(said) = crate::said("HYPRLAND_INSTANCE_SIGNATURE");
    let ours = said.unwrap_or_default();
    let Ok(instances) = instances();
    let Ok(runtime) = runtime();

    Ok(instances
        .into_iter()
        .filter(|name| *name != ours)
        .filter(|name| {
            let Ok(mut asking) = Program::Hyprctl.command();

            !asking
                .args(["-i", name, "version"])
                .output()
                .is_ok_and(|done| done.status.success())
        })
        .map(|name| runtime.join("hypr").join(name))
        .collect())
}

pub struct Starting {
    held: Option<File>,
}

impl Starting {
    pub fn now() -> Result<Self, Never> {
        let Ok(stages) = stages();
        let _ = std::fs::create_dir_all(&stages);

        let held = match File::create(stages.join("starting.lock")) {
            Ok(file) => Some(file),
            Err(fault) => {
                eprintln!("console-desktop: the lock two sessions start under: {fault}");

                None
            }
        };

        match &held {
            Some(file) => {
                // SAFETY: the descriptor is this file's, and open for the call.
                unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
            }
            None => {},
        }

        Ok(Starting { held })
    }
}

impl Drop for Starting {
    fn drop(&mut self) {
        match &self.held {
            Some(file) => {
                // SAFETY: as above, and this is the handle that took it.
                unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
            }
            None => {},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_compositor_running_this_screen_is_never_one_of_the_dead() {
        let ours = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default();
        if ours.is_empty() {
            return;
        }
        let dead = dead_instances().expect("the dead");
        assert!(!dead.iter().any(|path| path.ends_with(&ours)));
    }

    #[test]
    fn a_socket_named_after_a_display_is_not_the_display() {
        let named = |name: &str| named(name).expect("a name is read");

        assert_eq!(named("wayland-1"), Names::ADisplay);
        assert_eq!(named("wayland-12"), Names::ADisplay);
        assert_eq!(named("wayland-1.lock"), Names::SomethingElse);
        assert_eq!(
            named("wayland-2-awww-daemon.sock"),
            Names::SomethingElse,
            "the wallpaper daemon opens this beside the display it draws on, and a session \
             waiting for one would take it for the compositor it just started"
        );
        assert_eq!(named("wayland-"), Names::SomethingElse);
        assert_eq!(named("waylandish"), Names::SomethingElse);
        assert_eq!(named("pipewire-0"), Names::SomethingElse);
    }

    #[test]
    fn a_stage_is_abandoned_only_when_the_session_it_names_has_ended() {
        let ours = crate::stage().expect("this session's stage");
        let abandoned = abandoned().expect("the abandoned stages");
        assert!(!abandoned.contains(&ours), "this session's own stage is not abandoned");
    }
}
