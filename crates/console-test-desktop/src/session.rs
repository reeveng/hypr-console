//! Which compositor is which, and waiting for one to arrive.
//!
//! A nested Hyprland picks its own signature and its own display and announces
//! neither. The signature is the directory it makes under `hypr/`, named for
//! the second it started in, so the one new name there is this one's --
//! `Starting` is what keeps that true, by letting one session come up at a
//! time. The display it writes down itself: `hyprland.lock` in that directory
//! is its pid and the `wayland-` name it bound, and that file is where
//! `hyprctl instances` reads both from as well.
//!
//! What this used to do was watch the runtime directory for a `wayland-` name
//! that had not been there before, which is only sound about a compositor
//! nobody killed. libwayland takes the lowest name whose lock is free and
//! unlinks whatever socket is sitting on it, so a compositor that was killed
//! leaves its name in that directory for good -- and killing one is exactly
//! what this does to a session it decides never came up. The next session
//! binds that name, arrives under a name that was already there, waits out its
//! patience, is killed for never coming up, and hands the same name to the one
//! after it. Run one at a time nothing shows: the lowest free name is the one
//! the session before just gave back, so it always reads as new. Run four at
//! once and a different pair of them fails every time, saying the compositor
//! never came up about compositors that were up and drawing.
//!
//! A name is still held to `wayland-` and a number and nothing else, because
//! the line read out of a file a compositor is in the middle of writing is
//! whatever happened to be flushed. A display is that shape; half a pid is
//! not.

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

const WROTE_DOWN: &str = "hyprland.lock";

pub fn display(signature: &str) -> Result<Option<String>, Never> {
    let Ok(runtime) = runtime();

    let said = match std::fs::read_to_string(
        runtime.join("hypr").join(signature).join(WROTE_DOWN),
    ) {
        Ok(said) => said,
        Err(_fault) => return Ok(None),
    };

    what_it_bound(&said)
}

pub fn what_it_bound(said: &str) -> Result<Option<String>, Never> {
    let name = match said.lines().nth(1) {
        Some(name) => name.trim().to_string(),
        None => return Ok(None),
    };

    let Ok(names) = named(&name);

    Ok(match names {
        Names::ADisplay => Some(name),
        Names::SomethingElse => None,
    })
}

fn until<T>(
    patience: Duration,
    mut look: impl FnMut() -> Option<T>,
) -> Result<Option<T>, Never> {
    let Ok(patience) = Patience::asking_every(patience, BREATH);

    console_waiting::found_handed(patience, &mut look, |look| Ok(look()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Came {
    pub signature: String,
    pub display: String,
}

pub fn wait_for_one(was: &BTreeSet<String>) -> Result<Option<Came>, Never> {
    until(COMING_UP, || {
        let Ok(instances) = instances();
        let Ok(runtime) = runtime();

        instances
            .difference(was)
            .filter(|name| runtime.join("hypr").join(name).join(".socket.sock").exists())
            .find_map(|name| {
                let Ok(said) = display(name);

                said.map(|display| Came { signature: name.clone(), display })
            })
    })
}

pub const A_LINE: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrote {
    Something,
    Nothing,
}

pub fn lines(at: &Path) -> Result<usize, Never> {
    Ok(match std::fs::read(at) {
        Ok(read) => read.iter().filter(|byte| **byte == b'\n').count(),
        Err(_fault) => NOT_A_LINE,
    })
}

const NOT_A_LINE: usize = 0;

pub fn wait_for_more_than(at: &Path, already: usize, patience: Duration) -> Result<Wrote, Never> {
    let Ok(found) = until(patience, || {
        let Ok(now) = lines(at);

        match now > already {
            true => Some(()),
            false => None,
        }
    });

    Ok(match found {
        Some(()) => Wrote::Something,
        None => Wrote::Nothing,
    })
}

pub fn wait_for_written(at: &Path, patience: Duration) -> Result<Wrote, Never> {
    wait_for_more_than(at, NOT_A_LINE, patience)
}

pub fn left_behind(signature: &str) -> Result<(), Never> {
    let Ok(runtime) = runtime();
    let _ = std::fs::remove_dir_all(runtime.join("hypr").join(signature));

    Ok(())
}

pub fn swept() -> Result<(), Never> {
    let Ok(abandoned) = abandoned();

    for path in abandoned {
        let Ok(named) = crate::scope_of(&path);

        match named {
            Some(unit) => {
                let Ok(()) = console_program_lifetime::nothing_left_in(&unit);
            }
            None => {},
        }

        let _ = std::fs::remove_dir_all(path);
    }

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
            let name = match path.file_name() {
                Some(name) => name.to_string_lossy().to_string(),
                None => return false,
            };

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

#[cfg_attr(
    dylint_lib = "explicit029_no_asking_per_item",
    allow(
        explicit029_no_asking_per_item,
        reason = "a compositor is asked whether it is still there one at a time because there is nobody to ask about all of them at once, and the list is the sessions left behind on this machine"
    )
)]
pub fn dead_instances() -> Result<Vec<PathBuf>, Never> {
    let Ok(said) = console_compositor::instance();

    let ours = match said {
        Some(ours) => ours,
        None => String::new(),
    };
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

        #[cfg_attr(
            dylint_lib = "explicit040_no_torn_write",
            allow(
                explicit040_no_torn_write,
                reason = "the lock two nested desktops start under, whose whole point is the open file and not its bytes"
            )
        )]
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
            "the wallpaper daemon opens this beside the display it draws on, and it is \
             not the display"
        );
        assert_eq!(named("wayland-"), Names::SomethingElse);
        assert_eq!(named("waylandish"), Names::SomethingElse);
        assert_eq!(named("pipewire-0"), Names::SomethingElse);
    }

    #[test]
    fn a_compositor_says_which_display_it_bound_on_the_second_line() {
        let bound = |said: &str| what_it_bound(said).expect("what it wrote down");

        assert_eq!(bound("2015407\nwayland-1\n"), Some("wayland-1".to_string()));
        assert_eq!(bound("2015407\nwayland-12"), Some("wayland-12".to_string()));
    }

    #[test]
    fn a_lock_caught_half_written_names_no_display() {
        let bound = |said: &str| what_it_bound(said).expect("what it wrote down");

        assert_eq!(bound(""), None);
        assert_eq!(bound("2015407\n"), None, "the pid is written before the display is");
        assert_eq!(bound("2015407\nwayl"), None, "and the display is written a byte at a time");
    }

    #[test]
    fn a_stage_is_abandoned_only_when_the_session_it_names_has_ended() {
        let ours = crate::stage().expect("this session's stage");
        let abandoned = abandoned().expect("the abandoned stages");
        assert!(!abandoned.contains(&ours), "this session's own stage is not abandoned");
    }
}
