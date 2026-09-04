// UI test for EXPLICIT012 — `unsafe { … }` must have a `// SAFETY: …` comment.
//
// The dylint test harness reads `//~ EXPLICIT012_SAFETY_DOC` annotations on the
// line of the diagnostic. Lines without annotations must not produce
// diagnostics.

// BAD: unsafe block without a SAFETY comment.
//~v EXPLICIT012_SAFETY_DOC
fn no_safety_doc(p: *const u8) -> u8 {
    unsafe { *p }
}

// GOOD: SAFETY comment present.
fn with_safety_doc(p: *const u8) -> u8 {
    // SAFETY: the caller guarantees `p` is non-null and points to readable memory.
    unsafe { *p }
}

// GOOD: a non-unsafe block requires no comment.
fn safe_block() -> u32 {
    let x = 1;
    x + 1
}

fn main() {}

// GOOD: the reason runs to a second line, and the word is on the first.
fn multi_line_safety_doc(p: *const u8) -> u8 {
    // SAFETY: the caller guarantees `p` is non-null, and that nothing else
    // writes through it while this reads.
    unsafe { *p }
}

// BAD: a comment above, but no reason in it.
fn commented_but_unjustified(p: *const u8) -> u8 {
    // Read the byte.
    unsafe { *p }
}
