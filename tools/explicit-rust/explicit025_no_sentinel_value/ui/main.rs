// UI test for EXPLICIT025 — a number standing in for a case the type does not
// have.

// BAD EXPLICIT025 — the end of the type, meaning `no limit`.
fn endless(held: usize) -> bool {
    //~v EXPLICIT025_NO_SENTINEL_VALUE
    held == usize::MAX
}

// BAD EXPLICIT025 — minus one, meaning `not found`.
fn missing(at: i32) -> bool {
    //~v EXPLICIT025_NO_SENTINEL_VALUE
    at == -1
}

// BAD EXPLICIT025 — the same meaning, handed back rather than compared.
fn nowhere() -> i32 {
    //~v EXPLICIT025_NO_SENTINEL_VALUE
    -1
}

// BAD EXPLICIT025 — and again, with the word `return` in front of it.
fn nowhere_either(found: Option<i32>) -> i32 {
    match found {
        Some(at) => at,
        //~v EXPLICIT025_NO_SENTINEL_VALUE
        None => return -1,
    }
}

// GOOD — the case the type has.
fn found(at: usize) -> Option<usize> {
    Some(at)
}

pub enum Limit {
    Endless,
    Of(usize),
}

// GOOD — the case named as a variant, and counted by the compiler.
fn limited(held: Limit) -> usize {
    match held {
        Limit::Endless => 0,
        Limit::Of(how_many) => how_many,
    }
}

// GOOD — two numbers compared as numbers.
fn same(left: usize, right: usize) -> bool {
    left == right
}

// GOOD — a negative float beside a comparison is geometry, not a meaning.
fn onscreen(at: f32) -> bool {
    at > -1.0
}

fn main() {
    let _ = endless(2);
    let _ = missing(2);
    let _ = nowhere();
    let _ = nowhere_either(None);
    let _ = found(2);
    let _ = limited(Limit::Endless);
    let _ = same(2, 3);
    let _ = onscreen(2.0);
}
