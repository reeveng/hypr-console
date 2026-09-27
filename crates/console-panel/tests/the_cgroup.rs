//! A launched application is in a cgroup of its own, not the controller's.  The
//! menu, the panel, the music panel and the file manager are all started by
//! `console_panel::running::left_running`. Each one of them is a process whose
//! life is a single press, and what is started under it (the player, the
//! browser, the file viewer) is a process that has to outlive it. Under cgroup
//! v2 a child inherits its parent's cgroup, so without an explicit move the
//! launched program sits in `console-input-controller.service`'s cgroup.
//! Restarting the controller then takes the program with it, which is the harm
//! the entry in the backlog describes.  `left_running` wraps the program in
//! `systemd-run --user --scope`, which moves the child into a transient scope
//! unit named `run-<pid>-<id>.scope`. This test runs the same wrap against
//! `/bin/sleep` and reads the child's cgroup path back out of
//! `/proc/<pid>/cgroup`, asserting the path ends in a scope rather than the
//! controller's slice.  Skipped, not failed, on a machine without `systemd-run`
//! or without a user systemd to talk to. Both are common in a CI environment
//! and neither is a reason to fail the rest of the suite.

use std::process::{Command, Stdio};
use std::time::Duration;

use console_core_never::Never;
use console_program_lifetime::{Scopes, in_a_scope_of_its_own, scopes};

type Failure = Box<dyn std::error::Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Moved {
    Yes,
    No,
}

const HELD: &str = "5";

const MOVES: Duration = Duration::from_secs(5);

#[test]
fn a_launched_program_is_in_a_scope_of_its_own() -> Result<(), Failure> {
    let Ok(available) = scopes();

    match available {
        Scopes::Available => {}
        Scopes::Unavailable => {
            eprintln!("skipped: no systemd-run or no user systemd to talk to; scopes cannot be made");

            return Ok(());
        }
    }

    let arguments = vec![String::from("sleep"), String::from(HELD)];
    let Ok((_, wrapped)) = in_a_scope_of_its_own(None, &arguments);
    let (program, rest) = wrapped.split_first().ok_or("the scope wrapped the program in nothing")?;
    let spawned = Command::new(program).args(rest).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(_unstarted) => {
            eprintln!("skipped: {program} would not start, so there is nothing in a scope to ask about");

            return Ok(());
        }
    };
    let pid = child.id();
    let Ok(cgroup) = wait_for_cgroup(pid);
    let _ = child.kill();
    let _ = child.wait();
    let cgroup = cgroup.ok_or(format!(
        "/proc/{pid}/cgroup could not be read, though the program was given {HELD} seconds \
         to be there. Something ended it early, and this test has asked nothing."
    ))?;

    assert_eq!(
        moved(&cgroup),
        Ok(Moved::Yes),
        "the launched program is in the parent's cgroup, not a run-p scope: {cgroup}"
    );

    Ok(())
}

fn moved(cgroup: &str) -> Result<Moved, Never> {
    let segment = match cgroup.rsplit('/').next() {
        Some(segment) => segment,
        None => "",
    };

    Ok(match segment.starts_with("run-") && segment.ends_with(".scope") {
        true => Moved::Yes,
        false => Moved::No,
    })
}

fn wait_for_cgroup(pid: u32) -> Result<Option<String>, Never> {
    let Ok(patience) = console_waiting::Schedule::of(MOVES);
    let found = console_waiting::until_some(patience, || {
        let Ok(now) = read_cgroup(pid);

        Ok(now.filter(|now| moved(now) == Ok(Moved::Yes)))
    });

    match found {
        Ok(Some(found)) => Ok(Some(found)),
        Ok(None) => read_cgroup(pid),
    }
}

fn read_cgroup(pid: u32) -> Result<Option<String>, Never> {
    let at = format!("/proc/{pid}/cgroup");
    let said = match std::fs::read_to_string(at) {
        Ok(said) => said,
        Err(_ended) => return Ok(None),
    };

    Ok(said.lines().find(|line| line.starts_with("0::")).map(|v2| v2.trim_start_matches("0::").to_string()))
}
