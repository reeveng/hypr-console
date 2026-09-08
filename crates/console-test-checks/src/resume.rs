//! The windows that were open are open again.
//!
//! `console-resume` is the one feature on this desktop whose whole subject is
//! what happens between two compositors, so it is the one check that cannot be
//! written as a thing done to a screen and then looked at. It is two nested
//! sessions: the first has a window in it and is saved, the second starts with
//! nothing and is asked to put it back. Nothing in the second session opens a
//! terminal, so a terminal in the second session was put there by the thing
//! under test.
//!
//! What this stage can answer is that the windows come back. Where they land it
//! cannot: a window is moved onto the workspace it was saved from by the loop
//! that listens for windows opening, and what that listens to is this desktop's
//! own event pool, which the nested session does not run. On a screen with one
//! workspace there is nothing for that half to get wrong anyway. It is the
//! device's question and `todos.md` says so.
//!
//! ## It saves into a directory of its own, under the name the unit uses
//!
//! `CONSOLE_RESUME_PATH` is what the program takes for where sessions live, so
//! the check hands it a directory of its own and takes it away at the end. The
//! session inside it is called `default`, because `default` is what the unit
//! saves and this check is about what the unit does -- the rule that a check may
//! not touch somebody's `default` is about the person's sessions, and none of
//! these are.
//!
//! What it cannot point somewhere of its own is `XDG_RUNTIME_DIR`, which is
//! where the mark that says this compositor has been put back already lives.
//! hyprctl finds the compositor through the same variable, so a keeper with a
//! runtime directory of its own is a keeper that cannot see the desktop at all.
//! The marks are left where they belong, and the ones a run made are taken away
//! by name afterwards: they are keyed to a compositor that has stopped, and a
//! check may take away what it made.
//!
//! ## What a second start would take is a window nobody saved
//!
//! Counting the windows after a second start says nothing, because a keeper
//! that sweeps the screen puts the same session back on it and the count comes
//! out where it started. What tells them apart is a window that is *not* in the
//! session: the check opens one after the putting back has finished, so the
//! screen holds one window the file knows and one it does not, and a second
//! start that sweeps comes back with only the first.
//!
//! That is also why the keeper that puts back is told to save an hour from now.
//! Left on the interval the check saves with, it would write the second window
//! into the session while the check was still setting it up, and then a sweep
//! would put both back and the count would say nothing again. Neither number is
//! a wait: what the check waits for is each step saying it has happened.
//!
//! ## A word it does not know is pressed here, where there is no desktop to lose
//!
//! The other half of this feature is what the program does with a word nobody
//! wrote it for, and that half needs no compositor at all -- which is as well,
//! because the way it fails is by sweeping the screen of whoever ran it. So it
//! is pressed with no compositor within reach: the runtime directory it is
//! handed is one of the check's own and empty, so hyprctl can find nothing to
//! talk to, and `HYPRLAND_INSTANCE_SIGNATURE` is taken away, which is the
//! answer `already` refuses to act on. Either of those alone means a fallen-
//! through mode closes no windows, and the check has both.
//!
//! What it presses is the whole program rather than the reading of its words.
//! That the words parse is a unit test in the crate; that a word nobody knows
//! reaches a status and not a mode is only true if `main` wires it that way,
//! and the day it did not was the day `cargo run -- --help` closed every window
//! on this laptop.
//!
//! ## The keeper is told to save often
//!
//! It saves when it hears a window open and otherwise when the interval comes
//! round, and what it hears window openings through is the event pool the nested
//! session does not run. So the interval is the one that has to arrive, and it
//! is asked for a second rather than the minute the unit uses. The check does
//! not wait on that number: it waits for the session file to have a line in it,
//! and if the file never comes the check says the session was never written,
//! which is a true sentence about the feature.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use console_compositor::Window;
use console_core_never::Never;
use console_program_lifetime::{Alongside, Still};
use console_test_stages::checking::{Body, Check, Done, cannot, failed, same};
use console_test_stages::desktop::{Desktop, Installed};
use console_test_stages::here::Here;
use console_waiting::{Patience, Seen, Waited};

const TERMINAL: &str = "alacritty";

const CLASS: &str = "Alacritty";

const SAVED: &str = "default";

const SAVING: &str = "--save-interval=1";

const NOT_SAVING_AGAIN: &str = "--save-interval=3600";

const ASKING_AGAIN: &str = "0.2";

const TWO: usize = 2;

const ONE: usize = 1;

const KEEPER: &str = "console-resume";

const UNKNOWN: &str = "--help";

const REFUSING: Duration = Duration::from_secs(5);

const ASKING: Duration = Duration::from_millis(50);

pub const REFUSED: Check = Check {
    name: "360-a-word-the-session-keeper-does-not-know",
    about: "A word the session keeper does not know is refused rather than read as no word at all.",
    feature: "resume",
    since: "2026-09-07",
    bodies: &[Body::Here(refused)],
};

pub const AGAIN: Check = Check {
    name: "370-the-windows-that-were-open",
    about: "A window open when the desktop went away is open again when it comes back.",
    feature: "resume",
    since: "2026-09-07",
    bodies: &[Body::Desktop(again)],
};

pub const NOT_TWICE: Check = Check {
    name: "380-a-restart-is-not-a-desktop-starting",
    about: "The session keeper started again leaves the windows that are on the screen.",
    feature: "resume",
    since: "2026-09-07",
    bodies: &[Body::Desktop(not_twice)],
};

fn ours(name: &str) -> Result<PathBuf, String> {
    let at = std::env::temp_dir().join(format!("console-resume-{name}-{}", std::process::id()));

    let _ = std::fs::remove_dir_all(&at);

    std::fs::create_dir_all(&at).map_err(|fault| format!("{}: making it: {fault}", at.display()))?;

    Ok(at)
}

fn keeping(at: &Path, how_often: &str) -> Result<String, Never> {
    Ok(format!("CONSOLE_RESUME_PATH={} console-resume {how_often}", at.display()))
}

fn saying(at: &Path, how_often: &str, said: &Path) -> Result<String, Never> {
    let Ok(keeping) = keeping(at, how_often);

    Ok(format!("{keeping} >> {} 2>&1", said.display()))
}

fn when(there: &Path, then: &str) -> Result<String, Never> {
    Ok(format!("until [ -s {} ]; do sleep {ASKING_AGAIN}; done; {then}", there.display()))
}

fn marked() -> Result<PathBuf, String> {
    let runtime = std::env::var("XDG_RUNTIME_DIR")
        .map_err(|fault| format!("XDG_RUNTIME_DIR: {fault}"))?;

    Ok(PathBuf::from(runtime).join(console_resume::OURS))
}

fn marks() -> Result<BTreeSet<PathBuf>, String> {
    let at = marked()?;

    let read = match std::fs::read_dir(&at) {
        Ok(read) => read,
        Err(_nothing_has_been_put_back_here) => return Ok(BTreeSet::new()),
    };

    Ok(read.flatten().map(|entry| entry.path()).collect())
}

fn unmark(before: &BTreeSet<PathBuf>) -> Result<(), Never> {
    let after = match marks() {
        Ok(after) => after,
        Err(_nothing_says_where_the_marks_are) => return Ok(()),
    };

    for at in after.difference(before) {
        let _ = std::fs::remove_file(at);
    }

    Ok(())
}

fn refused(_stage: &mut Here) -> Done {
    let Ok(keeper) = console_test_stages::beside(KEEPER);
    let at = ours("refused")?;

    let mut command = Command::new(&keeper);

    command
        .arg(UNKNOWN)
        .env("CONSOLE_RESUME_PATH", at.join("sessions"))
        .env("XDG_RUNTIME_DIR", &at)
        .env_remove("HYPRLAND_INSTANCE_SIGNATURE")
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut running = match console_program_lifetime::alongside(&mut command) {
        Ok(running) => running,
        Err(fault) => return failed(format!("{}: {fault}", keeper.display())),
    };

    let Ok(patience) = Patience::asking_every(REFUSING, ASKING);

    let Ok(waited) = console_waiting::until(patience, || {
        let Ok(still) = running.still();

        Ok(match still {
            Still::Ended => Seen::Yes,
            Still::Running => Seen::NotYet,
        })
    });

    let done = ended(running, waited);

    let _ = std::fs::remove_dir_all(&at);

    done
}

fn ended(mut running: Alongside, waited: Waited) -> Done {
    match waited {
        Waited::RanOut => failed(format!(
            "{KEEPER} {UNKNOWN} was still running after {}s, so the word was read as no word at \
             all -- and no word at all is the mode that closes every window",
            REFUSING.as_secs()
        )),
        Waited::Happened => {
            let how = match running.waiting() {
                Ok(how) => how,
                Err(fault) => return failed(format!("what {KEEPER} ended as: {fault}")),
            };

            match how.success() {
                true => failed(format!(
                    "{KEEPER} {UNKNOWN} ended saying all was well, so a word nobody wrote it \
                     for is a word it takes"
                )),
                false => Ok(()),
            }
        },
    }
}

fn terminal(saying: &Path) -> Result<String, Never> {
    Ok(format!("{TERMINAL} -e sh -c 'echo up > {}; exec sh'", saying.display()))
}

fn terminals(open: &[Window]) -> Result<usize, Never> {
    Ok(open.iter().filter(|window| window.first_class == CLASS).count())
}

fn saving(stage: &mut Desktop, at: &Path) -> Done {
    let Ok(keeping) = keeping(at, SAVING);

    stage.open(TERMINAL)?;
    stage.open(&keeping)?;
    stage.not_before(&at.join(SAVED).join("exec.conf"))?;

    let open = stage.windows()?;
    let Ok(many) = terminals(&open);

    same(&many, &ONE, || {
        format!(
            "the session was saved with {many} terminals in it, so what it is about to be asked \
             to put back is not what this check opened"
        )
    })
}

fn again(stage: &mut Desktop) -> Done {
    let Ok(installed) = stage.installed(TERMINAL);

    match installed {
        Installed::No => return cannot("alacritty is not installed on this machine"),
        Installed::Yes => {},
    }

    let at = ours("came-back")?;
    let before = marks()?;

    let done = saved_and_put_back(stage, &at);

    let Ok(()) = unmark(&before);
    let _ = std::fs::remove_dir_all(&at);

    done
}

fn saved_and_put_back(stage: &mut Desktop, at: &Path) -> Done {
    saving(stage, at)?;

    let Ok(keeping) = keeping(at, NOT_SAVING_AGAIN);
    let Ok(()) = stage.fresh();

    stage.open(&keeping)?;

    let open = stage.windows()?;
    let Ok(many) = terminals(&open);

    same(&many, &ONE, || {
        format!(
            "a desktop that came up with nothing on it has {many} terminals on it after the \
             session keeper ran, where the session it was handed has one"
        )
    })
}

fn not_twice(stage: &mut Desktop) -> Done {
    let Ok(installed) = stage.installed(TERMINAL);

    match installed {
        Installed::No => return cannot("alacritty is not installed on this machine"),
        Installed::Yes => {},
    }

    let at = ours("not-twice")?;
    let before = marks()?;

    let done = put_back_and_started_again(stage, &at);

    let Ok(()) = unmark(&before);
    let _ = std::fs::remove_dir_all(&at);

    done
}

fn put_back_and_started_again(stage: &mut Desktop, at: &Path) -> Done {
    saving(stage, at)?;

    let put_back = at.join("put-back");
    let opened = at.join("opened");
    let again = at.join("again");

    let Ok(first) = saying(at, NOT_SAVING_AGAIN, &put_back);
    let Ok(second) = saying(at, NOT_SAVING_AGAIN, &again);
    let Ok(terminal) = terminal(&opened);
    let Ok(nobody_saved_it) = when(&put_back, &terminal);
    let Ok(started_again) = when(&opened, &second);

    let Ok(()) = stage.fresh();

    stage.open(&first)?;
    stage.open(&nobody_saved_it)?;
    stage.open(&started_again)?;
    stage.not_before(&again)?;

    let open = stage.windows()?;
    let Ok(many) = terminals(&open);

    same(&many, &TWO, || {
        format!(
            "the session keeper started a second time on a desktop it had already put back, and \
             the screen went from two terminals to {many}. The one it opened itself was in the \
             session; the other was not, so a screen swept and rebuilt out of the file comes \
             back with only the first"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_compositor::{Filling, Floating, Pinned};

    fn window(class: &str) -> Window {
        Window {
            address: "0x1".to_string(),
            title: "a shell".to_string(),
            first_class: class.to_string(),
            first_title: "a shell".to_string(),
            workspace: 1,
            workspace_named: "1".to_string(),
            monitor: Some(0),
            floating: Floating::No,
            pinned: Pinned::No,
            filling: Filling::Nothing,
            at: (0, 0),
            size: (800, 600),
            pid: 42,
        }
    }

    #[test]
    fn the_terminals_are_counted_and_nothing_else_is() {
        let open = [window(CLASS), window("waybar"), window(CLASS)];

        assert_eq!(terminals(&open), Ok(2));
        assert_eq!(terminals(&[]), Ok(0));
    }

    #[test]
    fn the_keeper_is_told_where_to_keep_the_session_and_how_often() {
        let Ok(keeping) = keeping(Path::new("/tmp/somewhere"), SAVING);

        assert!(keeping.starts_with("CONSOLE_RESUME_PATH=/tmp/somewhere "), "{keeping}");
        assert!(keeping.ends_with("console-resume --save-interval=1"), "{keeping}");
    }

    #[test]
    fn the_keeper_that_is_putting_back_is_not_the_one_that_saves() {
        let Ok(saving) = keeping(Path::new("/tmp/somewhere"), SAVING);
        let Ok(putting_back) = keeping(Path::new("/tmp/somewhere"), NOT_SAVING_AGAIN);

        assert_ne!(
            saving, putting_back,
            "a keeper that goes on saving while the check presses it saves the window the check \
             opened next, and the second start then has two to put back"
        );
    }

    #[test]
    fn nothing_is_started_until_the_thing_it_waits_for_is_there() {
        let Ok(waited) = when(Path::new("/tmp/said"), "alacritty");

        assert!(waited.starts_with("until [ -s /tmp/said ];"), "{waited}");
        assert!(waited.ends_with("; alacritty"), "{waited}");
    }

    #[test]
    fn the_second_terminal_says_when_its_shell_is_up_rather_than_when_it_was_asked_for() {
        let Ok(terminal) = terminal(Path::new("/tmp/two"));

        assert!(terminal.contains("echo up > /tmp/two"), "{terminal}");
        assert!(terminal.starts_with(TERMINAL), "{terminal}");
    }
}
