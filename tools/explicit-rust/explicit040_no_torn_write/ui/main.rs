// UI test for EXPLICIT040 — a file is written whole or not at all, and there
// is one place that knows how.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::Path;

// BAD EXPLICIT040 — this truncates the live file and then fills it.
fn straight_over(at: &Path, bytes: &[u8]) {
    //~v EXPLICIT040_NO_TORN_WRITE
    let _ = std::fs::write(at, bytes);
}

// BAD EXPLICIT040 — the live file is emptied before anything is written.
fn makes_it(at: &Path) {
    //~v EXPLICIT040_NO_TORN_WRITE
    let _ = File::create(at);
}

// BAD EXPLICIT040 — the options say it is about to be changed.
fn adds_to_it(at: &Path) {
    //~v EXPLICIT040_NO_TORN_WRITE
    let _ = OpenOptions::new().append(true).open(at);
}

// GOOD — reading is the other half of that crate and is not this rule.
fn reads_it(at: &Path) -> Option<String> {
    let mut held = File::open(at).ok()?;
    let mut said = String::new();

    held.read_to_string(&mut said).ok()?;

    Some(said)
}

// GOOD — the file is a log being appended to, and the site says so.
fn keeps_a_log(at: &Path) {
    #[cfg_attr(
        dylint_lib = "explicit040_no_torn_write",
        allow(
            explicit040_no_torn_write,
            reason = "a log is appended to and never rewritten, so a torn one loses the last line and nothing else"
        )
    )]
    let _ = OpenOptions::new().append(true).open(at);
}

fn main() {}
