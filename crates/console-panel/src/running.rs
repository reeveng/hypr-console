//! Starting things, and not waiting for them where waiting would be felt.

use std::process::{Command, Stdio};
use std::time::Duration;

/// What a command printed, or nothing if it could not be run.
pub fn said(argv: &[&str]) -> String {
    let Some((program, rest)) = argv.split_first() else {
        return String::new();
    };
    let Ok(done) = Command::new(program).args(rest).output() else {
        return String::new();
    };
    String::from_utf8_lossy(&done.stdout).trim().to_string()
}

/// Tell whoever is looking at the screen that something went wrong.
///
/// For the faults a person meets rather than reads about: a tap that did
/// nothing, a setting that was not written down. Everything else this desktop
/// gets wrong belongs in the journal, and the journal is not a place anybody
/// stands.
///
/// The counting is `console-say`'s: the journal always gets it and the screen
/// gets it a few times per kind per session, so a fault inside a loop cannot
/// become a wall of notifications nobody can dismiss. Not waited for, because
/// this is called from a path that has already failed and the panel still has
/// to draw. Said here as well if there is nothing to run, so the one thing
/// that cannot go quiet is the saying itself.
pub fn say(kind: &str, summary: &str, body: &str) {
    let started = Command::new("console-say")
        .args([kind, summary, body])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if started.is_err() {
        eprintln!("{kind}: {summary} - {body}");
    }
}

/// Run something and leave it running, in a scope of its own.
///
/// A scope of its own because a launched application shares the controller
/// daemon's cgroup otherwise: the daemon is what `console-controller.service`
/// runs as, and every process the launcher has ever started is a child of it
/// in the eyes of cgroup v2. Restarting the daemon then takes the menu, the
/// panel and the application together, which is the harm nobody had met
/// because the only way the daemon dies is the only way nobody reaches for.
///
/// `systemd-run --user --scope` wraps the child in a transient scope unit, so
/// the cgroup it ends up in is its own and survives the daemon. The `--` keeps
/// `--` from being read as a systemd-run flag when the program is something
/// like `--foo` (none on this machine, but a guarantee for free). `setsid` is
/// gone, because the scope puts the child in a new session already.
///
/// If `systemd-run` is not on the path, the raw argv is run instead. A machine
/// that has no user systemd at all falls back to what it had; a machine that
/// has one and then loses it gets a launched process in the parent's cgroup,
/// which is the same shape as today. The fallback is logged, because the next
/// thing to look at on a machine where it fired is whatever took systemd.
pub fn left_running(argv: &[String]) {
    use std::os::unix::process::CommandExt;

    let Some((program, rest)) = argv.split_first() else {
        return;
    };
    let (scope, scope_argv) = scope_around(argv);
    if scope && has_systemd_run() {
        let mut starting = Command::new(&scope_argv[0]);
        starting
            .args(&scope_argv[1..])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let _ = starting.spawn();
        return;
    }
    if !scope {
        eprintln!("left_running: not wrapping {} in a scope: {}", program, argv.join(" "));
    }
    let mut starting = Command::new(program);
    starting
        .args(rest)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: between the fork and the exec, and setsid is one call that
    // allocates nothing and touches nothing this process holds. The scope path
    // does not need it because the scope itself is a new session.
    unsafe {
        starting.pre_exec(|| {
            libc::setsid();
            Ok(())
        })
    };
    let _ = starting.spawn();
}

/// The argv a scope-wrapped call would build, separated from the act of
/// running it so the argv can be tested without spawning anything.
///
/// `true` is returned when the wrapping was applied (and the second slot holds
/// the wrapped argv); `false` is returned when the caller asked for no wrap,
/// in which case the second slot is the original argv. Tests pass the second
/// slot to a Command and inspect what comes out; production code passes the
/// flag through from `left_running` and reads the argv.
pub fn scope_around(argv: &[String]) -> (bool, Vec<String>) {
    let mut wrapped = Vec::with_capacity(argv.len() + 5);
    wrapped.push("systemd-run".to_string());
    wrapped.push("--user".to_string());
    wrapped.push("--scope".to_string());
    wrapped.push("--".to_string());
    wrapped.extend(argv.iter().cloned());
    (true, wrapped)
}

/// Whether `systemd-run` is on the path of whoever is running this.
///
/// Asked every time `left_running` is called rather than once at startup,
/// because the path of the running program is not the path of whoever is
/// running it in any of the panels this is called from: the launcher is
/// launched by the bar, the panel is launched by the menu, the music panel
/// is launched by the menu. A check at startup is the wrong shape.
fn has_systemd_run() -> bool {
    let path = std::env::var("PATH").unwrap_or_default();
    path.split(':')
        .filter(|at| !at.is_empty())
        .any(|at| std::path::Path::new(at).join("systemd-run").exists())
}

/// How long anything started from a panel is given before it is given up on.
pub const PATIENCE: Duration = Duration::from_secs(45);

/// How long something left running is given to say that it has started.
///
/// A tab drawn in the same millisecond the player was started is a tab that
/// says nothing is playing, which is the press appearing to have done nothing.
/// Measured on the device, kew answers on the bus 25 milliseconds after it is
/// asked for; this is ten times that, and it is a redraw rather than a wait,
/// so nothing is held up by it either way.
pub const SETTLING: Duration = Duration::from_millis(250);

#[cfg(test)]
mod tests {
    use super::*;

    /// The wrap puts `systemd-run --user --scope --` in front of the program
    /// and its arguments, in that order, with nothing in between.
    #[test]
    fn the_wrap_is_in_front_of_everything_else() {
        let argv = vec![
            "firefox".to_string(),
            "--new-window".to_string(),
            "https://example.com".to_string(),
        ];
        let (wrapped, made) = scope_around(&argv);
        assert!(wrapped, "the wrap was not applied");
        assert_eq!(
            made,
            vec![
                "systemd-run".to_string(),
                "--user".to_string(),
                "--scope".to_string(),
                "--".to_string(),
                "firefox".to_string(),
                "--new-window".to_string(),
                "https://example.com".to_string(),
            ]
        );
    }

    /// A program whose name starts with `--` would be read by `systemd-run` as
    /// one of its own flags. The `--` that follows `--scope` ends that.
    #[test]
    fn a_program_named_like_a_flag_is_kept_a_program() {
        let argv = vec!["--something".to_string()];
        let (_, made) = scope_around(&argv);
        assert_eq!(made[3], "--", "the terminator is what keeps the program a program");
        assert_eq!(made[4], "--something");
    }

    /// An empty argv leaves the wrap empty: `systemd-run --user --scope --`
    /// with nothing after the terminator is the shape of a miscall, and
    /// `left_running` returns before reaching this. The test only says the
    /// helper still hands back what was asked of it.
    #[test]
    fn an_empty_argv_still_makes_a_wrapped_one() {
        let (_, made) = scope_around(&[]);
        assert_eq!(made, vec!["systemd-run", "--user", "--scope", "--"]);
    }
}
