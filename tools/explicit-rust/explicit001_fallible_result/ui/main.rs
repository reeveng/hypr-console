// UI test for EXPLICIT001 — a failure met is a failure said.

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

// BAD EXPLICIT001 — saying somewhere that it can fail is not saying that it did.
fn says_it_can_fail_and_hides_that_it_did() -> Result<i32, ()> {
    //~v EXPLICIT001_FALLIBLE_RESULT
    Ok(fallible().unwrap_or(0))
}

// GOOD — both ways are named, and the failing one says what went wrong.
fn names_the_other_way() -> i32 {
    match fallible() {
        Ok(got) => got,
        Err(_nothing_to_read) => 0,
    }
}

// GOOD — propagated.
fn propagates() -> Result<i32, ()> {
    fallible()
}

fn main() {}
