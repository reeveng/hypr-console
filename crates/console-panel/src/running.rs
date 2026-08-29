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

/// How long the controller is given to answer before the panel draws anyway.
///
/// `controller-profile` waits a minute for InputPlumber to reach the bus, which
/// is right on a device booting and wrong here: on a machine that has no
/// InputPlumber at all it is a panel that appears a minute after it was asked
/// for, which is a panel nobody saw.
pub const A_WORD: Duration = Duration::from_secs(5);

/// Tell the controller which buttons this is, and open regardless.
///
/// The panel says this before it draws, so anything slow or missing here is a
/// menu that never appears. Told nothing, the buttons keep the meaning they
/// had, which is a menu you drive with the pointer rather than no menu.
pub fn controller(profile: &str) {
    let started = Command::new("controller-profile")
        .arg(profile)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut telling) = started else {
        eprintln!("no controller-profile to tell: the buttons are still the desktop's");
        return;
    };
    let by = std::time::Instant::now() + A_WORD;
    while std::time::Instant::now() < by {
        match telling.try_wait() {
            Ok(Some(how)) if how.success() => return,
            Ok(Some(how)) => {
                eprintln!("the controller would not take the {profile} buttons: {how}");
                return;
            }
            Err(fault) => {
                eprintln!("lost track of controller-profile: {fault}");
                return;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    let _ = telling.kill();
    let _ = telling.wait();
    eprintln!("controller-profile did not answer: the buttons are still the desktop's");
}

/// Run something and leave it running, in a session of its own.
///
/// A session of its own because a menu is a door rather than a parent: what
/// comes out of it goes on running when the menu that opened it has gone, and
/// nothing that happens to the menu's terminal is any of its business.
pub fn left_running(argv: &[String]) {
    use std::os::unix::process::CommandExt;

    let Some((program, rest)) = argv.split_first() else {
        return;
    };
    let mut starting = Command::new(program);
    starting
        .args(rest)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: between the fork and the exec, and setsid is one call that
    // allocates nothing and touches nothing this process holds.
    unsafe {
        starting.pre_exec(|| {
            libc::setsid();
            Ok(())
        })
    };
    let _ = starting.spawn();
}

/// How long anything started from a panel is given before it is given up on.
pub const PATIENCE: Duration = Duration::from_secs(45);
