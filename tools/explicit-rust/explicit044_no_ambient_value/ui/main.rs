// UI test for EXPLICIT044 — a function decides from what it was handed.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

// BAD EXPLICIT044 — filled once and frozen for the life of the process, so
// every later caller gets the first caller's answer.
//~v EXPLICIT044_NO_AMBIENT_VALUE
static ASKED: OnceLock<String> = OnceLock::new();

// BAD EXPLICIT044 — written by anything, at any moment.
//~v EXPLICIT044_NO_AMBIENT_VALUE
static STOPPING: AtomicBool = AtomicBool::new(false);

// GOOD — a table no one can write.
static NAMES: [&str; 2] = ["one", "other"];

// BAD EXPLICIT044 — this is wherever the program was started from.
fn from_where_it_was_started() -> Option<PathBuf> {
    //~v EXPLICIT044_NO_AMBIENT_VALUE
    std::env::current_dir().ok()
}

// BAD EXPLICIT044 — every relative path in the process means something else
// afterwards.
fn moves_the_floor(at: &Path) {
    //~v EXPLICIT044_NO_AMBIENT_VALUE
    let _ = std::env::set_current_dir(at);
}

// GOOD — the root is handed in and the path is joined to it.
fn under(root: &Path, named: &str) -> PathBuf {
    root.join(named)
}

// GOOD — the value is held by whoever the caller already holds.
struct Held {
    asked: Option<String>,
}

fn what_it_said(held: &Held) -> Option<&String> {
    held.asked.as_ref()
}

fn main() {
    let _ = STOPPING.load(Ordering::Relaxed);
    let _ = ASKED.get();
    let _ = NAMES.first();
}
