//! Only one chooser is ever up, and it is the last one asked for.
//!
//! The menu is on a button, on a paddle and on a key; the settings are on a
//! button and on four of the bar's icons. Every one of those roads starts a
//! process that knows nothing about the others, and two choosers at once take
//! each other's controller profile: the second to open claims it, the first to
//! close hands the desktop's buttons back while the other is still on screen.
//! Since both are drawn in the same place, backing out of one leaves you
//! looking at what appears to be the same chooser refusing to close.
//!
//! Turning the second one away instead would be worse in the one case that
//! matters: the bar is reachable with a finger while a chooser is up, and an
//! icon that does nothing at all reads as a broken bar. So the one on screen
//! goes. Ask through the door it came out of and nothing replaces it, which is
//! how a finger closes a panel it opened from the bar.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// The chooser this test is the second of.
const OTHER: &str = env!("CARGO_BIN_EXE_second-chooser");

/// How long the one that was up is given to go.
const PATIENCE: Duration = Duration::from_secs(5);

/// A session of this test's own, so that two of them running at once are not
/// each other's second chooser.
fn runtime(what: &str) -> PathBuf {
    let here = std::env::temp_dir().join(format!("console-lock-{}-{what}", std::process::id()));
    let _ = std::fs::remove_dir_all(&here);
    std::fs::create_dir_all(&here).expect("somewhere to keep a lock");
    here
}

/// Another process holding the screen, by the door called `name`.
fn already_up(runtime: &Path, name: &str) -> Child {
    holding(runtime, "hold", name)
}

/// Another process that has the screen and has not drawn on it yet.
fn on_its_way(runtime: &Path, name: &str) -> Child {
    holding(runtime, "coming", name)
}

/// Another process that took the screen and never draws on it.
fn stuck(runtime: &Path, name: &str) -> Child {
    holding(runtime, "stuck", name)
}

/// Another process whose window has gone, still holding the lock.
fn going(runtime: &Path, name: &str) -> Child {
    holding(runtime, "going", name)
}

fn holding(runtime: &Path, how: &str, name: &str) -> Child {
    let mut child = Command::new(OTHER)
        .args([how, name])
        .env("XDG_RUNTIME_DIR", runtime)
        .stdout(Stdio::piped())
        .spawn()
        .expect("a chooser");
    let mut said = String::new();
    BufReader::new(child.stdout.take().expect("its voice"))
        .read_line(&mut said)
        .expect("a word from it");
    assert_eq!(said.trim(), "held");
    child
}

/// What a chooser asking through this door is told.
fn asking(runtime: &Path, how: &str, name: &str) -> String {
    let done = Command::new(OTHER)
        .args([how, name])
        .env("XDG_RUNTIME_DIR", runtime)
        .output()
        .expect("an answer");
    String::from_utf8_lossy(&done.stdout).trim().to_string()
}

fn gone(up: &mut Child) -> bool {
    let by = Instant::now() + PATIENCE;
    while Instant::now() < by {
        if up.try_wait().is_ok_and(|ended| ended.is_some()) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = up.kill();
    false
}

/// The bar's speaker tapped twice: out, and away again. There is no B under a
/// finger, so this is the whole of how a panel opened from the bar is put back.
#[test]
fn the_door_that_opened_it_closes_it() {
    let runtime = runtime("same-door");
    let mut up = already_up(&runtime, "settings Sound");
    assert_eq!(asking(&runtime, "ask", "settings Sound"), "no");
    assert!(gone(&mut up), "the panel that was up is still up");
}

/// The bell names no tab, so its door is the panel's name and the space where a
/// tab would have been. What is written down is read back trimmed, so until the
/// name was trimmed on the way in as well the bell could not recognise its own
/// door: the tap put the panel away for being somebody else's and opened it
/// again in the same breath. Every other icon along that edge names a tab,
/// which is why the bell was the only one that flickered.
#[test]
fn a_door_that_names_no_tab_is_still_the_door_it_came_out_of() {
    let runtime = runtime("no-tab");
    let mut up = already_up(&runtime, "notices ");
    assert_eq!(asking(&runtime, "ask", "notices "), "no");
    assert!(gone(&mut up), "the bell opened again the panel it had just put away");
}

/// The battery tapped while the sound is up is one panel showing the battery,
/// not two panels or a tap that did nothing.
#[test]
fn another_door_takes_its_place() {
    let runtime = runtime("other-door");
    let mut up = already_up(&runtime, "settings Sound");
    assert_eq!(asking(&runtime, "ask", "settings Battery"), "yes");
    assert!(gone(&mut up), "two panels are up at once");
}

/// The one going hands the controller back on its way out, and the one arriving
/// takes it. In the wrong order that leaves a panel on screen with the desktop's
/// buttons under it. The lock is not free until the process holding it has
/// ended, so waiting for the lock is waiting for the hand-back to have happened.
#[test]
fn the_screen_is_taken_before_it_is_drawn_on() {
    let runtime = runtime("in-order");
    let mut up = already_up(&runtime, "menu");
    assert_eq!(asking(&runtime, "ask", "settings "), "yes");
    assert!(
        up.try_wait().is_ok_and(|ended| ended.is_some()),
        "it drew before the last one had gone"
    );
}

/// Asking is not taking. A chooser that checks again part way through would
/// otherwise refuse itself.
#[test]
fn the_one_that_holds_it_may_ask_twice() {
    let runtime = runtime("twice");
    assert_eq!(asking(&runtime, "twice", "menu"), "yes yes");
}

/// A lock outliving the process it was taken for is worse than the fault it
/// fixes: a menu that cannot be opened again until the session ends. The kernel
/// drops it when the process does, killed or not.
#[test]
fn a_chooser_that_dies_does_not_keep_the_lock() {
    let runtime = runtime("died");
    assert_eq!(asking(&runtime, "ask", "menu"), "yes");
    assert_eq!(asking(&runtime, "ask", "menu"), "yes");
}

/// The paddle pressed twice while the menu is still on its way is a thumb that
/// has not seen it yet. Closing what has not appeared is how one press became
/// two: the second cancelled the menu the first asked for, and the third was
/// the one that seemed to work.
#[test]
fn a_chooser_on_its_way_is_left_to_come() {
    let runtime = runtime("coming");
    let mut coming = on_its_way(&runtime, "menu");
    assert_eq!(asking(&runtime, "ask", "menu"), "no");
    assert!(
        coming.try_wait().is_ok_and(|ended| ended.is_none()),
        "the menu that was coming was cancelled by the press that waited for it"
    );
    let _ = coming.kill();
}

/// A chooser holds the lock until it has handed the desktop's buttons back,
/// which is after its window has gone. A press arriving in that last stretch
/// is somebody asking for the screen that is no longer showing anything, and
/// it used to be read as asking to close what was already closed: one press in
/// every open and close disappeared, and the next one was the one that seemed
/// to work.
#[test]
fn a_chooser_whose_window_has_gone_hands_the_screen_over() {
    let runtime = runtime("going");
    let mut last = going(&runtime, "menu");
    assert_eq!(asking(&runtime, "ask", "menu"), "yes");
    let _ = last.kill();
    let _ = last.wait();
}

/// Waiting for one on its way is only safe if the wait ends. A chooser that
/// takes the screen and hangs before it draws would otherwise be a desktop
/// where no button opens anything and nothing says why, until the session ends.
#[test]
fn a_chooser_that_never_draws_is_taken_over() {
    let runtime = runtime("stuck");
    let mut never = stuck(&runtime, "menu");
    assert_eq!(asking(&runtime, "ask", "menu"), "yes");
    let _ = never.kill();
    let _ = never.wait();
}
