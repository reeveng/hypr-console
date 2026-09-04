// UI test for EXPLICIT003 — `Result<T, !>` is forbidden.
#![feature(never_type)]

// BAD EXPLICIT003 — an error that cannot happen, written as though it could.
//~v EXPLICIT003_NO_NEVER_ERROR
fn cannot_fail() -> Result<i32, !> {
    Ok(0)
}

// GOOD — it cannot fail, so it does not say it can.
fn plainly(value: i32) -> i32 {
    value
}

// GOOD — a real error type, which says what went wrong.
fn reads() -> Result<String, std::io::Error> {
    Err(std::io::Error::other("read"))
}

fn main() {}
