//! What a fault says, and how often it says it.
//!
//! Everything `console-say` is called from is a loop of some kind: a service
//! that restarts, a daemon that comes round every five minutes, an apply that
//! walks a list. The first few notifications tell somebody something is wrong.
//! The two hundredth is a machine shouting over itself, and the way that ends
//! is with notifications turned off and the fault still there.
//!
//! So the journal gets everything and the screen gets a few, and that split is
//! what these assert. Run against the programs themselves, with a `notify-send`
//! and a `logger` of the test's own on the path, because what is worth knowing
//! is what the program does and not what libnotify does.
//!
//! These were written against two shell scripts and are unchanged in what they
//! ask. That is the whole reason for keeping them: the programs are new, and
//! what a person meets is meant not to be.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use console_external_programs::Program;

const SAY: &str = env!("CARGO_BIN_EXE_console-say");
const FELL: &str = env!("CARGO_BIN_EXE_console-fell");

const LOUD: usize = 5;

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
        let Ok(logger) = Program::Logger.name();

        listening.stub(logger, "written");
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

    fn run(&self, program: &str, argv: &[&str], result: Option<&str>) {
        let path = format!(
            "{}:{}",
            self.here.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut running = Command::new(program);
        running
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

#[test]
fn a_fault_that_keeps_happening_is_shown_a_few_times_and_written_down_every_time() {
    let listening = Listening::new("again");
    for _ in 0..LOUD + 3 {
        listening.say("wallpaper-choice", "The wallpaper was not changed");
    }
    assert_eq!(listening.counted("shown"), LOUD);
    assert_eq!(listening.counted("written"), LOUD + 3);
}

#[test]
fn the_last_one_shown_says_that_it_is_the_last() {
    let listening = Listening::new("last");
    for _ in 0..LOUD {
        listening.say("wallpaper-choice", "The wallpaper was not changed");
    }
    let shown = std::fs::read_to_string(listening.here.join("shown")).expect("something shown");
    let last = shown.lines().next_back().expect("a last one");
    assert!(
        last.contains("Not shown again"),
        "the last one shown said only: {last}"
    );
}

#[test]
fn two_kinds_of_fault_are_counted_apart() {
    let listening = Listening::new("kinds");
    for _ in 0..LOUD {
        listening.say("wallpaper-choice", "The wallpaper was not changed");
    }
    listening.say("compositor", "The compositor stopped answering");
    assert_eq!(listening.counted("shown"), LOUD + 1);
}

#[test]
fn a_service_that_was_asked_to_stop_says_nothing() {
    let listening = Listening::new("clean");
    listening.run(FELL, &["console-paper.service"], Some("success"));
    assert_eq!(listening.counted("shown"), 0);
    assert_eq!(listening.counted("written"), 0);
}

#[test]
fn a_service_that_fell_over_says_which_one_it_was() {
    let listening = Listening::new("fell");
    listening.run(FELL, &["console-paper.service"], Some("core-dump"));
    let shown = std::fs::read_to_string(listening.here.join("shown")).expect("something shown");
    assert!(
        shown.contains("console-paper.service"),
        "it did not name itself: {shown}"
    );

    let written = std::fs::read_to_string(listening.here.join("written")).expect("a line");
    assert!(
        written.contains("core-dump"),
        "the journal was not told what happened: {written}"
    );
}
