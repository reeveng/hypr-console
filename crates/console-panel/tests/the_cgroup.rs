//! A launched application is in a cgroup of its own, not the controller's.  The
//! menu, the panel, the music panel and the file manager are all started by
//! `console_panel::running::left_running`. Each one of them is a process whose
//! life is a single press, and what is started under it (the player, the
//! browser, the file viewer) is a process that has to outlive it. Under cgroup
//! v2 a child inherits its parent's cgroup, so without an explicit move the
//! launched program sits in `console-input-controller.service`'s cgroup.
//! Restarting the controller then takes the program with it, which is the harm
//! the entry in `todos.md` describes.  `left_running` wraps the program in
//! `systemd-run --user --scope`, which moves the child into a transient scope
//! unit named `run-<pid>-<id>.scope`. This test runs the same wrap against
//! `/bin/sleep` and reads the child's cgroup path back out of
//! `/proc/<pid>/cgroup`, asserting the path ends in a scope rather than the
//! controller's slice.  Skipped, not failed, on a machine without `systemd-run`
//! or without a user systemd to talk to. Both are common in a CI environment
//! and neither is a reason to fail the rest of the suite.

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use console_core_external_programs::Program;
use console_panel::running::scope_around;

#[test]
fn a_launched_program_is_in_a_scope_of_its_own() {
    let argv = match wrap("sleep", &[HELD]) {
        Some(argv) => argv,
        None => return,
    };
    let mut child = match run(&argv) {
        Some(child) => child,
        None => return,
    };
    let pid = child.id();
    let cgroup = settled(pid);
    let _ = child.kill();
    let _ = child.wait();
    let cgroup = match cgroup {
        Some(cgroup) => cgroup,
        None => {
            panic!(
                "/proc/{pid}/cgroup could not be read, though the program was given {HELD} seconds \
                 to be there. Something ended it early, and this test has asked nothing."
            )
        }
    };
    assert!(
        moved(&cgroup),
        "the launched program is in the parent's cgroup, not a run-p scope: {cgroup}"
    );
}

fn wrap(name: &str, args: &[&str]) -> Option<Vec<String>> {
    if !has_systemd_run() {
        eprintln!("skipped: no systemd-run on PATH; scopes cannot be made");
        return None;
    }
    if !has_user_systemd() {
        eprintln!("skipped: no user systemd to talk to; scopes cannot be made");
        return None;
    }
    let argv: Vec<String> = std::iter::once(name.to_string())
        .chain(args.iter().map(|word| (*word).to_string()))
        .collect();
    let Ok((_, wrapped)) = scope_around(&argv);
    Some(wrapped)
}

const HELD: &str = "5";

fn run(argv: &[String]) -> Option<Child> {
    Command::new(&argv[0])
        .args(&argv[1..])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

fn moved(cgroup: &str) -> bool {
    let segment = cgroup.rsplit('/').next().unwrap_or("");
    segment.starts_with("run-") && segment.ends_with(".scope")
}

const MOVES: Duration = Duration::from_secs(5);

fn settled(pid: u32) -> Option<String> {
    let until = Instant::now() + MOVES;
    let mut last = None;
    while Instant::now() < until {
        let now = read_cgroup(pid);
        if now.as_deref().is_some_and(moved) {
            return now;
        }
        last = now.or(last);
        std::thread::sleep(Duration::from_millis(50));
    }
    last
}

fn read_cgroup(pid: u32) -> Option<String> {
    let at = format!("/proc/{pid}/cgroup");
    let said = std::fs::read_to_string(at).ok()?;
    let v2 = said.lines().find(|line| line.starts_with("0::"))?;
    Some(v2.trim_start_matches("0::").to_string())
}

fn has_systemd_run() -> bool {
    let path = std::env::var("PATH").unwrap_or_default();
    path.split(':')
        .filter(|at| !at.is_empty())
        .any(|at| Path::new(at).join("systemd-run").exists())
}

fn has_user_systemd() -> bool {
    let Ok(mut asking) = Program::Systemctl.command();

    asking
        .args(["--user", "show", "-p", "Version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|how| how.success())
        .unwrap_or(false)
}
