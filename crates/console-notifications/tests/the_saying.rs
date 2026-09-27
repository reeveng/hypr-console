//! What a fault says, and how often it says it.
//!
//! Everything `console-say` is called from is a loop of some kind: a service
//! that restarts, a daemon that comes round every five minutes, an apply that
//! walks a list. The first few notifications tell someone something is wrong.
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

use std::error::Error;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use console_core_external_programs::Program;

const SAY: &str = env!("CARGO_BIN_EXE_console-say");
const REPORT_CRASH: &str = env!("CARGO_BIN_EXE_console-report-crash");

const LOUD: u32 = 5;

const AGAIN: (&str, &str) = ("wallpaper-choice", "The wallpaper was not changed");

type Failure = Box<dyn Error>;

struct Subscriber {
    here: PathBuf,
}

impl Subscriber {
    fn new(named: &str) -> Result<Self, Failure> {
        let here = console_core_temporary_directories::fresh(&format!("legion-saying-{named}"))?;

        std::fs::create_dir_all(here.join("bin"))?;
        std::fs::create_dir_all(here.join("run"))?;

        let listening = Subscriber { here };
        let Ok(logger) = Program::Logger.name();

        listening.stub(("notify-send", "shown"))?;
        listening.stub((logger, "written"))?;

        Ok(listening)
    }

    fn stub(&self, (program, into): (&str, &str)) -> Result<(), Failure> {
        let at = self.here.join("bin").join(program);
        let script = format!(
            "#!/bin/sh\necho \"$@\" >> {}\n",
            self.here.join(into).display()
        );

        console_core_atomic_writes::whole(&at, script.as_bytes())?;
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755))?;

        Ok(())
    }

    fn run(&self, program: &str, arguments: &[&str], result: Option<&str>) -> Result<(), Failure> {
        #[cfg_attr(
            dylint_lib = "explicit026_env_read_once",
            allow(
                explicit026_env_read_once,
                reason = "the program is run with the stubs in front of the path this test was given, so the path it was given is what is read"
            )
        )]
        let given = std::env::var("PATH")?;
        let path = format!("{}:{given}", self.here.join("bin").display());
        let mut running = Command::new(program);

        running
            .args(arguments)
            .env("PATH", path)
            .env("XDG_RUNTIME_DIR", self.here.join("run"));

        match result {
            Some(result) => {
                running.env("SERVICE_RESULT", result);
            }
            None => {}
        }

        let _status = running.status()?;

        Ok(())
    }

    fn say(&self, (kind, summary): (&str, &str)) -> Result<(), Failure> {
        self.run(SAY, &[kind, summary, "the body"], None)
    }

    fn read(&self, what: &str) -> Result<String, console_core_atomic_writes::Unread> {
        console_core_atomic_writes::text_or_empty(&self.here.join(what))
    }

    fn counted(&self, what: &str) -> Result<u32, Failure> {
        let said = self.read(what)?;
        let Ok(lines) = console_core_number_conversion::fitted::<_, u32>(said.lines().count());

        Ok(lines)
    }
}

impl Drop for Subscriber {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.here);
    }
}

#[test]
fn a_fault_that_keeps_happening_is_shown_a_few_times_and_written_down_every_time() -> Result<(), Failure> {
    let listening = Subscriber::new("again")?;
    let many = LOUD.saturating_add(3);

    for _ in 0..many {
        listening.say(AGAIN)?;
    }

    let shown = listening.counted("shown")?;
    let written = listening.counted("written")?;

    assert_eq!(shown, LOUD);
    assert_eq!(written, many);

    Ok(())
}

#[test]
fn the_last_one_shown_says_that_it_is_the_last() -> Result<(), Failure> {
    let listening = Subscriber::new("last")?;

    for _ in 0..LOUD {
        listening.say(AGAIN)?;
    }

    let shown = listening.read("shown")?;
    let last = shown.lines().next_back().ok_or("nothing was shown")?;

    assert!(
        last.contains("Not shown again"),
        "the last one shown said only: {last}"
    );

    Ok(())
}

#[test]
fn two_kinds_of_fault_are_counted_apart() -> Result<(), Failure> {
    let listening = Subscriber::new("kinds")?;

    for _ in 0..LOUD {
        listening.say(AGAIN)?;
    }

    listening.say(("compositor", "The compositor stopped answering"))?;

    let shown = listening.counted("shown")?;

    assert_eq!(shown, LOUD.saturating_add(1));

    Ok(())
}

#[test]
fn a_service_that_was_asked_to_stop_says_nothing() -> Result<(), Failure> {
    let listening = Subscriber::new("clean")?;

    listening.run(REPORT_CRASH, &["console-paper.service"], Some("success"))?;

    let shown = listening.counted("shown")?;
    let written = listening.counted("written")?;

    assert_eq!(shown, 0);
    assert_eq!(written, 0);

    Ok(())
}

#[test]
fn a_service_that_fell_over_says_which_one_it_was() -> Result<(), Failure> {
    let listening = Subscriber::new("crash")?;

    listening.run(REPORT_CRASH, &["console-paper.service"], Some("core-dump"))?;

    let shown = listening.read("shown")?;
    let written = listening.read("written")?;

    assert!(
        shown.contains("console-paper.service"),
        "it did not name itself: {shown}"
    );
    assert!(
        written.contains("core-dump"),
        "the journal was not told what happened: {written}"
    );

    Ok(())
}
