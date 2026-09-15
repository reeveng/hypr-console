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

// GOOD — a negative literal is a number written down and not a subtraction
// anybody performs; the compiler evaluates it, and a literal past the end of
// its own type fails the build.
fn below_zero() -> i32 {
    -1
}

// GOOD — const context: the compiler evaluates it, and overflow fails the
// build, which is a failure with a name.
const WIDTH: usize = 16 * 4;

// GOOD — a `NonZero` divisor is the policy, said in the type. Division's one
// failure is the divisor being zero, and this is the proof that it is not.
fn wrapped(at: usize, many: std::num::NonZeroUsize) -> usize {
    at % many
}

// GOOD — the same, divided rather than remaindered.
fn shared(over: usize, whole: std::num::NonZeroUsize) -> usize {
    over / whole
}

// BAD EXPLICIT015 — an ordinary divisor proves nothing about itself.
fn shared_unproven(over: usize, whole: usize) -> usize {
    //~v EXPLICIT015_NO_BARE_ARITHMETIC
    over / whole
}

fn main() {
    let whole = std::num::NonZeroUsize::new(4);
    let _ = (WIDTH, grows(0), splits(4, 2), grows_named(0), scales(1.0), below_zero());
    let _ = whole.map(|whole| (wrapped(5, whole), shared(8, whole)));
    let _ = shared_unproven(8, 4);

    let mut sum = 0;
    accumulates(&mut sum, 1);
}
