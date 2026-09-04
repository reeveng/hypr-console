// UI test for EXPLICIT001 — a function that swallows a failure must say so
// in its return type.

fn fallible() -> Result<i32, ()> {
    Ok(0)
}

// BAD EXPLICIT001 — it met a failure and told nobody.
fn swallows() -> i32 {
    //~v EXPLICIT001_FALLIBLE_RESULT
    fallible().unwrap_or(0)
}

// BAD EXPLICIT001 — same, by another name.
fn asks_and_forgets() -> bool {
    //~v EXPLICIT001_FALLIBLE_RESULT
    fallible().is_ok()
}

// GOOD — it swallows the failure, and its signature says it may fail anyway,
// so the caller is not the one being kept in the dark.
fn allowed_to_choose() -> Result<i32, ()> {
    Ok(fallible().unwrap_or(0))
}

// GOOD — propagated.
fn propagates() -> Result<i32, ()> {
    fallible()
}

fn main() {}
