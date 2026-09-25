// UI test for EXPLICIT001 — a failure met is a failure said.

fn fallible() -> Result<i32, ()> {
    Ok(0)
}

// BAD EXPLICIT001 — it met a failure and told no one.
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

// BAD EXPLICIT001 — `unwrap_or(0)` with the words moved apart.
fn swallows_in_an_arm() -> i32 {
    match fallible() {
        Ok(got) => got,
        //~v EXPLICIT001_FALLIBLE_RESULT
        Err(_) => 0,
    }
}

// BAD EXPLICIT001 — an early return is still a value made up.
fn returns_a_value_in_an_arm() -> Result<i32, ()> {
    let got = match fallible() {
        Ok(got) => got,
        //~v EXPLICIT001_FALLIBLE_RESULT
        Err(..) => return Ok(0),
    };
    Ok(got)
}

// BAD EXPLICIT001 — hidden behind an or-pattern.
fn swallows_beside_an_absence(held: Result<Option<i32>, ()>) -> i32 {
    match held {
        Ok(Some(got)) => got,
        //~v EXPLICIT001_FALLIBLE_RESULT
        Ok(None) | Err(_) => 0,
    }
}

// BAD EXPLICIT001 — a name that only says there was a failure.
fn names_it_a_fault() -> i32 {
    match fallible() {
        Ok(got) => got,
        //~v EXPLICIT001_FALLIBLE_RESULT
        Err(_fault) => 0,
    }
}

// GOOD — the failing arm hands a fault on.
fn hands_a_fault_on() -> Result<i32, String> {
    match fallible() {
        Ok(got) => Ok(got),
        Err(_) => Err("nothing to read".to_string()),
    }
}

// GOOD — the same, returned early.
fn returns_a_fault() -> Result<i32, String> {
    let got = match fallible() {
        Ok(got) => got,
        Err(_) => return Err("nothing to read".to_string()),
    };
    Ok(got)
}

enum Never {}

fn cannot_fail() -> Result<Option<i32>, Never> {
    Ok(None)
}

// GOOD — a `Result` that cannot fail has no failure to swallow.
fn nothing_to_swallow() -> i32 {
    match cannot_fail() {
        Ok(Some(got)) => got,
        Ok(None) | Err(_) => 0,
    }
}

// GOOD — propagated.
fn propagates() -> Result<i32, ()> {
    fallible()
}

fn main() {}
