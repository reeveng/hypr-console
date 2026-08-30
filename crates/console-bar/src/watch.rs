//! What wakes a reading up.
//!
//! Each of these has something that says when it changed, so nothing here polls
//! for the sake of it: the sound is told by pipewire, the network by
//! NetworkManager, and every one of them by the compositor when a panel opens
//! over it. The tick under them is the net, for a machine where one of those
//! is not running and for the battery, which nothing announces.

use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use console_again::keep;
use console_panel::door::watching_layers;

use crate::reading::What;

/// How often the bell is counted when nothing said anything.
///
/// The net under the monitor below, and nothing more: every notification that
/// arrives or goes says so, so this is only what carries the reading on a
/// machine where the monitor could not be started.
pub const BELL: Duration = Duration::from_secs(10);

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
///
/// It reconnects for as long as anything is listening, which is what keeps an
/// icon lighting after the socket has been away. See
/// `console_panel::door::watching_layers`.
fn layers(say: Sender<()>) {
    watching_layers(say);
}

/// A word whenever a notification arrived or went, or the panel opened over it.
///
/// busctl is asked to watch the name mako owns, which catches both halves: the
/// call that raises a notification, and the signal that says one has closed,
/// whether a thumb took it down or it ran out of seconds. Nothing else on the
/// bus is watched, so a desktop doing anything at all does not wake this.
///
/// The compositor is the other half, as it is for every reading here: the bell
/// lights while the panel it opens is in front, and a layer opening is the
/// only thing that says so. Left off, the bell lit up to ten seconds after the
/// tap that opened it, which reads as an icon that does not answer.
///
/// stdbuf, because a monitor whose output is a pipe buffers it by the
/// kilobyte, and a bell that lights when the buffer fills has not lit.
pub fn watching_notices() -> Receiver<()> {
    let (say, heard) = channel();
    layers(say.clone());
    lines(
        vec!["stdbuf", "-oL", "busctl", "--user", "monitor", "org.freedesktop.Notifications"],
        say,
    );
    heard
}

/// A program that prints a line whenever the thing it watches changed.
///
/// It is asked to die with this one. waybar restarts a module that exits, and
/// a `pactl subscribe` left behind by each of those is a wake-up a second for
/// the rest of the session; twenty-five of them were found alive on the device
/// once, the oldest four hours old.
///
/// It is started again whenever it ends. These programs are subscriptions to
/// other daemons, and a daemon restarting takes its subscribers down with it:
/// pipewire coming back after a resume ends every `pactl subscribe` on the
/// machine, and NetworkManager does the same to `nmcli monitor`. Started once,
/// the reading behind it went from something the machine announced to
/// something noticed on the next tick, ten or thirty seconds later, for the
/// rest of the session. Nothing reported that as broken, because it was not:
/// it was slow, and only ever slow after something else had happened.
pub fn lines(argv: Vec<&'static str>, say: Sender<()>) {
    keep(move || once(&argv, &say));
}

/// One run of it, and whether anybody still wants another.
///
/// False only when nothing is listening any more. A program that would not
/// start is true: it may be a daemon that is not up yet, and the waiting above
/// is what that costs.
fn once(argv: &[&'static str], say: &Sender<()>) -> bool {
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
        return true;
    };
    let Some(out) = running.stdout.take() else {
        let _ = running.kill();
        let _ = running.wait();
        return true;
    };
    for _ in BufReader::new(out).lines().map_while(Result::ok) {
        if say.send(()).is_err() {
            let _ = running.kill();
            let _ = running.wait();
            return false;
        }
    }
    // Its output ended, so it is on its way out or already gone. Waited for
    // rather than left: this starts another one every time round, and a child
    // nobody asks after stays as a zombie.
    let _ = running.kill();
    let _ = running.wait();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole of what a watcher is for. `echo` prints its line and ends,
    /// which is every one of these programs on the day the daemon behind it
    /// restarts. Run against a watcher that started its program once, this
    /// hears the first word and then waits until the timeout.
    #[test]
    fn a_watcher_whose_program_ends_is_started_again() {
        let (say, heard) = channel();
        lines(vec!["echo", "something happened"], say);
        for word in 1..=2 {
            heard
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|_| panic!("word {word} of 2"));
        }
    }

    /// And it stops when nobody is listening, rather than starting a
    /// `pactl subscribe` every second for the rest of the session on behalf of
    /// a reading that has gone.
    #[test]
    fn a_watcher_nobody_is_listening_to_stops() {
        let (say, heard) = channel::<()>();
        drop(heard);
        assert!(!once(&["echo", "anything"], &say));
    }

    /// A program that is not on this machine is a daemon that might yet be
    /// started, not a reason to give up on the reading for the session.
    #[test]
    fn a_program_that_will_not_start_is_worth_another_try() {
        let (say, _heard) = channel::<()>();
        assert!(once(&["console-nothing-is-called-this"], &say));
    }
}
