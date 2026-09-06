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

const OTHER: &str = env!("CARGO_BIN_EXE_second-chooser");

const PATIENCE: Duration = Duration::from_secs(5);

fn runtime(what: &str) -> PathBuf {
    let here = std::env::temp_dir().join(format!("console-lock-{}-{what}", std::process::id()));
    let _ = std::fs::remove_dir_all(&here);
    std::fs::create_dir_all(&here).expect("somewhere to keep a lock");
    here
}

fn already_up(runtime: &Path, name: &str) -> Child {
    holding(runtime, "hold", name)
}

fn on_its_way(runtime: &Path, name: &str) -> Child {
    holding(runtime, "coming", name)
}

fn stuck(runtime: &Path, name: &str) -> Child {
    holding(runtime, "stuck", name)
}

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

#[test]
fn the_door_that_opened_it_closes_it() {
    let runtime = runtime("same-door");
    let mut up = already_up(&runtime, "settings Sound");
    assert_eq!(asking(&runtime, "ask", "settings Sound"), "no");
    assert!(gone(&mut up), "the panel that was up is still up");
}

#[test]
fn a_door_that_names_no_tab_is_still_the_door_it_came_out_of() {
    let runtime = runtime("no-tab");
    let mut up = already_up(&runtime, "notices ");
    assert_eq!(asking(&runtime, "ask", "notices "), "no");
    assert!(gone(&mut up), "the bell opened again the panel it had just put away");
}

#[test]
fn another_door_takes_its_place() {
    let runtime = runtime("other-door");
    let mut up = already_up(&runtime, "settings Sound");
    assert_eq!(asking(&runtime, "ask", "settings Battery"), "yes");
    assert!(gone(&mut up), "two panels are up at once");
}

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

#[test]
fn the_one_that_holds_it_may_ask_twice() {
    let runtime = runtime("twice");
    assert_eq!(asking(&runtime, "twice", "menu"), "yes yes");
}

#[test]
fn a_chooser_that_dies_does_not_keep_the_lock() {
    let runtime = runtime("died");
    assert_eq!(asking(&runtime, "ask", "menu"), "yes");
    assert_eq!(asking(&runtime, "ask", "menu"), "yes");
}

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

#[test]
fn a_chooser_whose_window_has_gone_hands_the_screen_over() {
    let runtime = runtime("going");
    let mut last = going(&runtime, "menu");
    assert_eq!(asking(&runtime, "ask", "menu"), "yes");
    let _ = last.kill();
    let _ = last.wait();
}

#[test]
fn a_chooser_that_never_draws_is_taken_over() {
    let runtime = runtime("stuck");
    let mut never = stuck(&runtime, "menu");
    assert_eq!(asking(&runtime, "ask", "menu"), "yes");
    let _ = never.kill();
    let _ = never.wait();
}
