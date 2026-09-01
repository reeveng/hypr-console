//! What the screen says while the machine is being rebuilt under it.
//!
//! An apply is a minute of writing files, restarting services and compiling
//! every program the manifest names, and for all of it the screen used to say
//! nothing. So "is the thing I am about to press the new one?" was a question
//! answered by remembering how long ago the deploy went, and a fault reported
//! against a copy that had already been replaced costs an evening at both ends
//! of the wire.
//!
//! What matters is that there is one notice and not a pile of them: it is
//! raised with no expiry so it stands for however long the apply takes, and
//! every later call replaces that same notice rather than adding a line under
//! one that never goes. So these assert the id going out and coming back.
//!
//! Run against the script in the tree with a `notify-send` of the test's own,
//! because what is worth knowing is what the script does and not what
//! libnotify does.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

const UPDATING: &str = env!("CARGO_BIN_EXE_console-updating");

/// The number the stub hands back for every notification it is asked for,
/// which is what `--print-id` gives a caller on a real machine.
const ID: &str = "7";

/// Somewhere to be said to: a stub notify-send, and a runtime directory for
/// the program to keep the notification's number in.
struct Listening {
    here: PathBuf,
}

impl Listening {
    fn new(named: &str) -> Self {
        let named = format!("console-updating-{named}-{}", std::process::id());
        let here = std::env::temp_dir().join(named);
        let _ = std::fs::remove_dir_all(&here);
        std::fs::create_dir_all(here.join("bin")).expect("somewhere to listen");
        std::fs::create_dir_all(here.join("run")).expect("somewhere to count");
        let at = here.join("bin/notify-send");
        let script = format!(
            "#!/bin/sh\necho \"$@\" >> {}\necho {ID}\n",
            here.join("shown").display()
        );
        std::fs::write(&at, script).expect("a stub");
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755)).expect("runnable");
        Listening { here }
    }

    fn run(&self, word: &str) {
        let path = format!(
            "{}:{}",
            self.here.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        Command::new(UPDATING)
            .arg(word)
            .env("PATH", path)
            .env("XDG_RUNTIME_DIR", self.here.join("run"))
            .status()
            .expect("it runs");
    }

    fn shown(&self) -> Vec<String> {
        std::fs::read_to_string(self.here.join("shown"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn kept(&self) -> String {
        std::fs::read_to_string(self.here.join("run/console/updating")).unwrap_or_default()
    }
}

impl Drop for Listening {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.here);
    }
}

/// The whole point. One notice stands for as long as the apply does, and the
/// one that says it finished is that same notice said again, not a second one
/// underneath a first that never goes.
#[test]
fn the_notice_that_says_it_finished_replaces_the_one_that_said_it_started() {
    let listening = Listening::new("replaces");
    listening.run("start");
    listening.run("done");
    let shown = listening.shown();
    assert_eq!(shown.len(), 2, "two notifications, not {}: {shown:?}", shown.len());
    assert!(
        !shown[0].contains("--replace-id"),
        "the first replaced something that was not there: {}",
        shown[0]
    );
    assert!(
        shown[1].contains(&format!("--replace-id={ID}")),
        "the second was a new notification rather than the same one: {}",
        shown[1]
    );
}

/// It has no expiry, because an apply takes as long as it takes and a notice
/// that goes at five seconds is a notice that says nothing about the minute
/// after it.
#[test]
fn the_one_that_stands_while_the_apply_runs_does_not_time_out() {
    let listening = Listening::new("standing");
    listening.run("start");
    let shown = listening.shown();
    assert!(
        shown[0].contains("--expire-time=0"),
        "it would have gone by itself: {}",
        shown[0]
    );
}

/// The number is kept only while there is something up to replace. Left
/// behind, the next apply would replace a notification that had already gone.
#[test]
fn the_number_is_kept_while_the_notice_stands_and_let_go_when_it_does_not() {
    let listening = Listening::new("kept");
    listening.run("start");
    assert_eq!(listening.kept().trim(), ID);
    listening.run("done");
    assert_eq!(listening.kept(), "", "the number outlived the notification");
}

/// An apply that stopped halfway leaves a machine that is neither what it was
/// nor what it was going to be. That is worth more than four seconds of
/// somebody's attention, so it stays up and it says so loudly.
#[test]
fn an_apply_that_did_not_finish_says_so_and_stays_on_the_screen() {
    let listening = Listening::new("failed");
    listening.run("start");
    listening.run("failed");
    let said = listening.shown().pop().expect("something shown");
    assert!(said.contains("--urgency=critical"), "said quietly: {said}");
    assert!(said.contains("--expire-time=0"), "it would have gone by itself: {said}");
    assert!(said.contains("did not finish"), "it did not say what happened: {said}");
}

/// A word this does not know is a mistake at the call site, and the call sites
/// are an apply. Saying so beats raising a notice nobody can read.
#[test]
fn a_word_it_does_not_know_is_refused() {
    let listening = Listening::new("unknown");
    listening.run("sideways");
    assert!(listening.shown().is_empty(), "it showed something for a word it does not know");
}
