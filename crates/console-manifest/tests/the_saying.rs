//! What a fault says, and how often it says it.
//!
//! Everything `legion-say` is called from is a loop of some kind: a service
//! that restarts, a daemon that comes round every five minutes, an apply that
//! walks a list. The first few notifications tell somebody something is wrong.
//! The two hundredth is a machine shouting over itself, and the way that ends
//! is with notifications turned off and the fault still there.
//!
//! So the journal gets everything and the screen gets a few, and that split is
//! what these assert. Run against the scripts in the tree, with a
//! `notify-send` and a `logger` of the test's own on the path, because what is
//! worth knowing is what the script does and not what libnotify does.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const SAY: &str = "files/usr/local/bin/console-say";
const FELL: &str = "files/usr/local/bin/console-fell";

/// How many times one kind of fault reaches the screen. The script's own
/// number, written here so that changing it means changing both.
const LOUD: usize = 5;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository")
}

/// Somewhere to be said to: a stub for each program the script reaches for,
/// and a runtime directory for it to count in.
struct Listening {
    here: PathBuf,
}

impl Listening {
    fn new(named: &str) -> Self {
        let named = format!("legion-saying-{named}-{}", std::process::id());
        let here = std::env::temp_dir().join(named);
        let _ = std::fs::remove_dir_all(&here);
        std::fs::create_dir_all(here.join("bin")).expect("somewhere to listen");
        std::fs::create_dir_all(here.join("run")).expect("somewhere to count");
        let listening = Listening { here };
        listening.stub("notify-send", "shown");
        listening.stub("logger", "written");
        listening
    }

    fn stub(&self, program: &str, into: &str) {
        let at = self.here.join("bin").join(program);
        let script = format!(
            "#!/bin/sh\necho \"$@\" >> {}\n",
            self.here.join(into).display()
        );
        std::fs::write(&at, script).expect("a stub");
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755)).expect("runnable");
    }

    fn run(&self, script: &str, argv: &[&str], result: Option<&str>) {
        // The stubs first, then the tree's own scripts, because `legion-fell`
        // reaches for `legion-say` by name exactly as it does on the device.
        let path = format!(
            "{}:{}:{}",
            self.here.join("bin").display(),
            root().join("files/usr/local/bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut running = Command::new("sh");
        running
            .arg(root().join(script))
            .args(argv)
            .env("PATH", path)
            .env("XDG_RUNTIME_DIR", self.here.join("run"));
        if let Some(result) = result {
            running.env("SERVICE_RESULT", result);
        }
        running.status().expect("it runs");
    }

    fn say(&self, kind: &str, summary: &str) {
        self.run(SAY, &[kind, summary, "the body"], None);
    }

    fn counted(&self, what: &str) -> usize {
        std::fs::read_to_string(self.here.join(what))
            .unwrap_or_default()
            .lines()
            .count()
    }
}

impl Drop for Listening {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.here);
    }
}

/// The whole point: a fault that happens for ever is written down for ever and
/// shown a handful of times.
#[test]
fn a_fault_that_keeps_happening_is_shown_a_few_times_and_written_down_every_time() {
    let listening = Listening::new("again");
    for _ in 0..LOUD + 3 {
        listening.say("wallpaper-choice", "The wallpaper was not changed");
    }
    assert_eq!(listening.counted("shown"), LOUD);
    assert_eq!(listening.counted("written"), LOUD + 3);
}

/// The last one shown says it is the last one, because a notification that
/// simply stops is a fault that looks fixed.
#[test]
fn the_last_one_shown_says_that_it_is_the_last() {
    let listening = Listening::new("last");
    for _ in 0..LOUD {
        listening.say("wallpaper-choice", "The wallpaper was not changed");
    }
    let shown = std::fs::read_to_string(listening.here.join("shown")).expect("something shown");
    let last = shown.lines().next_back().expect("a last one");
    assert!(
        last.contains("last time"),
        "the last one shown said only: {last}"
    );
}

/// Counted by kind, so a compositor that has gone quiet does not use up what
/// the wallpaper had to say.
#[test]
fn two_kinds_of_fault_are_counted_apart() {
    let listening = Listening::new("kinds");
    for _ in 0..LOUD {
        listening.say("wallpaper-choice", "The wallpaper was not changed");
    }
    listening.say("compositor", "The compositor stopped answering");
    assert_eq!(listening.counted("shown"), LOUD + 1);
}

/// Every service runs this as it stops, and almost every stop is the target
/// going down at logout. A notification for each of those is the surest way to
/// have all of them ignored.
#[test]
fn a_service_that_was_asked_to_stop_says_nothing() {
    let listening = Listening::new("clean");
    listening.run(FELL, &["console-paper.service"], Some("success"));
    assert_eq!(listening.counted("shown"), 0);
    assert_eq!(listening.counted("written"), 0);
}

/// One that fell over says so, and names itself, because the notification is
/// read by somebody who is not going to run `systemctl` afterwards.
#[test]
fn a_service_that_fell_over_says_which_one_it_was() {
    let listening = Listening::new("fell");
    listening.run(FELL, &["console-paper.service"], Some("core-dump"));
    let shown = std::fs::read_to_string(listening.here.join("shown")).expect("something shown");
    assert!(
        shown.contains("console-paper.service"),
        "it did not name itself: {shown}"
    );
    assert!(
        shown.contains("core-dump"),
        "it did not say what happened: {shown}"
    );
}
