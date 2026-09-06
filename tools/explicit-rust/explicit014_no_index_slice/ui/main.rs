// UI test for EXPLICIT014 — indexing and slicing are forbidden.

// BAD EXPLICIT014 — an element that may not be there, taken without asking.
fn nth(xs: &[u8], i: usize) -> u8 {
    //~v EXPLICIT014_NO_INDEX_SLICE
    xs[i]
}

// BAD EXPLICIT014 — a slice is the same question about a range.
fn head(s: &str) -> &str {
    //~v EXPLICIT014_NO_INDEX_SLICE
    &s[0..1]
}

// GOOD — the absent element is a case with a name.
fn nth_named(xs: &[u8], i: usize) -> Option<u8> {
    xs.get(i).copied()
}

// GOOD — const context: the compiler evaluates it, and an index out of range
// fails the build, which is a failure with a name.
const EVERY: [u8; 3] = [1, 2, 3];
const SECOND: u8 = EVERY[1];

fn main() {
    let _ = SECOND;
}
