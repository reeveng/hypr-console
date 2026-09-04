// UI test for EXPLICIT005 — a `Result` used as a statement is a failure
// nobody handled.

fn fallible() -> Result<i32, ()> {
    Ok(0)
}

// BAD EXPLICIT005 — the failure is dropped where it stands.
fn drops_it() {
    //~v EXPLICIT005_HANDLE_FALLIBLE
    fallible();
}

// GOOD — propagated.
fn propagates() -> Result<(), ()> {
    let _value = fallible()?;
    Ok(())
}

// GOOD — handled.
fn handles() {
    match fallible() {
        Ok(_) => {}
        Err(()) => {}
    }
}

// GOOD — discarded on purpose, in writing. EXPLICIT009 is about that spelling.
fn says_it_does_not_care() {
    let _ = fallible();
}

fn main() {}
