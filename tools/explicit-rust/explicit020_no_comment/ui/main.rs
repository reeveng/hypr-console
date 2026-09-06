//! The fixture for EXPLICIT020, which is the only one in this suite that has
//! to be written under its own rule: every line of prose in it is a comment,
//! so the prose is here, in the head, where the rule allows it.
//!
//! A line that is meant to be caught carries its own `//~` annotation, which
//! the harness reads and the rule reports -- one diagnostic, on the line that
//! is also asking for it. A line with no annotation must draw nothing.

fn plain() -> u32 {
    // what this is doing //~ EXPLICIT020_NO_COMMENT
    1
}

/// what this returns //~ EXPLICIT020_NO_COMMENT
fn documented() -> u32 {
    2
}

/* said in a block */ //~ EXPLICIT020_NO_COMMENT
fn blocked() -> u32 {
    3
}

fn trailing() -> u32 {
    4 // the answer //~ EXPLICIT020_NO_COMMENT
}

fn read(p: *const u8) -> u8 {
    // SAFETY: the caller guarantees `p` points at a byte that may be read.
    unsafe { *p }
}

fn read_at_length(p: *const u8) -> u8 {
    // SAFETY: the caller guarantees `p` points at a byte that may be read,
    // and that nothing writes through it while this reads. A reason may run
    // past its first line, and the run is allowed by the line that names it.
    unsafe { *p }
}

fn said_twice(p: *const u8) -> u8 {
    // SAFETY: the caller guarantees `p` points at a byte that may be read.

    // and this paragraph is not part of that reason //~ EXPLICIT020_NO_COMMENT
    unsafe { *p }
}

fn slashes_in_a_string() -> &'static str {
    "https://example.invalid/ is not a comment"
}

fn main() {
    let _ = plain();
    let _ = documented();
    let _ = blocked();
    let _ = trailing();
    let _ = slashes_in_a_string();
}
