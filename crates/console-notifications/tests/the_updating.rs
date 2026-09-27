//! What the screen says while the machine is being rebuilt under it.
//!
//! An apply is a minute of writing files, restarting services and compiling
//! every program the manifest names, and for all of it the screen used to say
//! nothing. So "is the thing I am about to press the new one?" was a question
//! answered by remembering how long ago the deploy went, and a fault reported
//! against a copy that had already been replaced costs an evening at both ends
//! of the wire.
//!
//! What matters is that there is one notification and not a pile of them: it is
//! raised with no expiry so it stands for however long the apply takes, and
//! every later call replaces that same notification rather than adding a line under
//! one that never goes. So these assert the id going out and coming back.
//!
//! Run against the script in the tree with a `notify-send` of the test's own,
//! because what is worth knowing is what the script does and not what
//! libnotify does.

use std::error::Error;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

const UPDATING: &str = env!("CARGO_BIN_EXE_console-updating");

const ID: &str = "7";

type Failure = Box<dyn Error>;

struct Subscriber {
    here: PathBuf,
}

impl Subscriber {
    fn new(named: &str) -> Result<Self, Failure> {
        let here = console_core_temporary_directories::fresh(&format!("console-updating-{named}"))?;

        std::fs::create_dir_all(here.join("bin"))?;
        std::fs::create_dir_all(here.join("run"))?;

        let at = here.join("bin/notify-send");
        let script = format!(
            "#!/bin/sh\necho \"$@\" >> {}\necho {ID}\n",
            here.join("shown").display()
        );

        console_core_atomic_writes::whole(&at, script.as_bytes())?;
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755))?;

        Ok(Subscriber { here })
    }

    fn run(&self, word: &str) -> Result<(), Failure> {
        #[cfg_attr(
            dylint_lib = "explicit026_env_read_once",
            allow(
                explicit026_env_read_once,
                reason = "the program is run with the stub in front of the path this test was given, so the path it was given is what is read"
            )
        )]
        let given = std::env::var("PATH")?;
        let path = format!("{}:{given}", self.here.join("bin").display());
        let _status = Command::new(UPDATING)
            .arg(word)
            .env("PATH", path)
            .env("XDG_RUNTIME_DIR", self.here.join("run"))
            .status()?;

        Ok(())
    }

    fn shown(&self) -> Result<Vec<String>, console_core_atomic_writes::Unread> {
        let shown = console_core_atomic_writes::text_or_empty(&self.here.join("shown"))?;

        Ok(shown.lines().map(str::to_string).collect())
    }

    fn kept(&self) -> Result<String, console_core_atomic_writes::Unread> {
        console_core_atomic_writes::text_or_empty(&self.here.join("run/console/updating"))
    }
}

impl Drop for Subscriber {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.here);
    }
}

#[test]
fn the_notification_that_says_it_finished_replaces_the_one_that_said_it_started() -> Result<(), Failure> {
    let listening = Subscriber::new("replaces")?;

    listening.run("start")?;
    listening.run("done")?;

    let shown = listening.shown()?;

    match shown.as_slice() {
        [started, finished] => {
            assert!(
                !started.contains("--replace-id"),
                "the first replaced something that was not there: {started}"
            );
            assert!(
                finished.contains(&format!("--replace-id={ID}")),
                "the second was a new notification rather than the same one: {finished}"
            );

            Ok(())
        }
        other => Err(Box::from(format!("two notifications, not {}: {other:?}", other.len()))),
    }
}

#[test]
fn the_one_that_stands_while_the_apply_runs_does_not_time_out() -> Result<(), Failure> {
    let listening = Subscriber::new("standing")?;

    listening.run("start")?;

    let shown = listening.shown()?;
    let standing = shown.first().ok_or("nothing was shown")?;

    assert!(
        standing.contains("--expire-time=0"),
        "it would have gone by itself: {standing}"
    );

    Ok(())
}

#[test]
fn the_number_is_kept_while_the_notification_stands_and_let_go_when_it_does_not() -> Result<(), Failure> {
    let listening = Subscriber::new("kept")?;

    listening.run("start")?;

    let standing = listening.kept()?;

    assert_eq!(standing.trim(), ID);

    listening.run("done")?;

    let finished = listening.kept()?;

    assert_eq!(finished, "", "the number outlived the notification");

    Ok(())
}

#[test]
fn an_apply_that_did_not_finish_says_so_and_stays_on_the_screen() -> Result<(), Failure> {
    let listening = Subscriber::new("failed")?;

    listening.run("start")?;
    listening.run("failed")?;

    let mut shown = listening.shown()?;
    let said = shown.pop().ok_or("nothing was shown")?;

    assert!(said.contains("--urgency=critical"), "said quietly: {said}");
    assert!(said.contains("--expire-time=0"), "it would have gone by itself: {said}");
    assert!(said.contains("didn't finish"), "it did not say what happened: {said}");

    Ok(())
}

#[test]
fn a_word_it_does_not_know_is_refused() -> Result<(), Failure> {
    let listening = Subscriber::new("unknown")?;

    listening.run("sideways")?;

    let shown = listening.shown()?;

    assert!(shown.is_empty(), "it showed something for a word it does not know");

    Ok(())
}
