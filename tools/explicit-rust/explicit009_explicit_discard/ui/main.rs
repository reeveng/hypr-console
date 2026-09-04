// UI test for EXPLICIT009 — a discarded `#[must_use]` value must be
// discarded in writing.

#[must_use]
fn counted() -> i32 {
    1
}

// BAD EXPLICIT009 — thrown away silently.
fn silent() {
    //~v EXPLICIT009_EXPLICIT_DISCARD
    counted();
}

// GOOD — the discard is a decision somebody wrote down.
fn deliberate() {
    let _ = counted();
}

// GOOD — used.
fn used() -> i32 {
    counted() + 1
}

fn main() {}
