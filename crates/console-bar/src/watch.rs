//! What wakes a reading up.
//!
//! Each of these has something that says when it changed, so nothing here polls
//! for the sake of it: the sound is told by pipewire, the network by
//! NetworkManager, and every one of them by the compositor when a panel opens
//! over it. The tick under them is the net, for a machine where one of those
//! is not running and for the battery, which nothing announces.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use console_panel::door::{events, worth_asking_after};

use crate::reading::What;

/// How often a reading is taken when nothing said anything.
pub fn tick(what: What) -> Duration {
    match what {
        What::Battery => Duration::from_secs(30),
        What::Bluetooth => Duration::from_secs(10),
        What::Network => Duration::from_secs(10),
        What::Sound => Duration::from_secs(10),
    }
}

/// What says a reading changed, where anything does.
fn teller(what: What) -> Option<Vec<&'static str>> {
    match what {
        What::Battery => None,
        What::Bluetooth => None,
        What::Network => Some(vec!["nmcli", "monitor"]),
        What::Sound => Some(vec!["pactl", "subscribe"]),
    }
}

/// A word whenever anything that could change the answer happened.
pub fn watching(what: What) -> Receiver<()> {
    let (say, heard) = channel();
    layers(say.clone());

    if let Some(argv) = teller(what) {
        lines(argv, say);
    }
    heard
}

/// The compositor, which is how the icon knows a panel is over it.
fn layers(say: Sender<()>) {
    std::thread::spawn(move || {
        let Some(socket) = events() else { return };
        let Ok(stream) = UnixStream::connect(&socket) else { return };
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if worth_asking_after(&line) && say.send(()).is_err() {
                return;
            }
        }
    });
}

/// A program that prints a line whenever the thing it watches changed.
///
/// It is asked to die with this one. waybar restarts a module that exits, and
/// a `pactl subscribe` left behind by each of those is a wake-up a second for
/// the rest of the session; twenty-five of them were found alive on the device
/// once, the oldest four hours old.
fn lines(argv: Vec<&'static str>, say: Sender<()>) {
    std::thread::spawn(move || {
        let mut asking = Command::new(argv[0]);
        asking.args(&argv[1..]).stdout(Stdio::piped()).stderr(Stdio::null());
        // SAFETY: one call, to a function that is async-signal-safe, between
        // the fork and the exec.
        unsafe {
            asking.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
        let Ok(mut running) = asking.spawn() else {
            return;
        };
        let Some(out) = running.stdout.take() else { return };
        for _ in BufReader::new(out).lines().map_while(Result::ok) {
            if say.send(()).is_err() {
                let _ = running.kill();
                return;
            }
        }
    });
}
