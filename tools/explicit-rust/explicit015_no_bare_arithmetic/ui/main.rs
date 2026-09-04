// UI test for EXPLICIT015 — bare arithmetic on integers is forbidden.

// BAD EXPLICIT015 — panics in debug, wraps in release, says neither.
fn grows(count: u32) -> u32 {
    //~v EXPLICIT015_NO_BARE_ARITHMETIC
    count + 1
}

// BAD EXPLICIT015 — division panics on zero in both profiles.
fn splits(total: u32, ways: u32) -> u32 {
    //~v EXPLICIT015_NO_BARE_ARITHMETIC
    total / ways
}

// BAD EXPLICIT015 — compound assignment is the same operator in other clothes.
fn accumulates(sum: &mut u64, next: u64) {
    //~v EXPLICIT015_NO_BARE_ARITHMETIC
    *sum += next;
}

// GOOD — the policy has a name, and what comes back is met.
fn grows_named(count: u32) -> u32 {
    count.saturating_add(1)
}

// GOOD — floats neither panic nor wrap; they are not this rule's business.
fn scales(x: f64) -> f64 {
    x * 2.0
}

// GOOD — const context: the compiler evaluates it, and overflow fails the
// build, which is a failure with a name.
const WIDTH: usize = 16 * 4;

fn main() {
    let _ = (WIDTH, grows(0), splits(4, 2), grows_named(0), scales(1.0));

    let mut sum = 0;
    accumulates(&mut sum, 1);
}
