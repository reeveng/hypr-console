// UI test for EXPLICIT004 — `unwrap`, `expect`, `panic!`, `todo!`,
// `unimplemented!` and `unreachable!` are forbidden. The four macros are
// caught on the macro backtrace, so all four are tried here: matching what
// they lower to had quietly stopped catching three of them.

// BAD EXPLICIT004 — `unwrap`
//~v EXPLICIT004_NO_PANIC
fn use_unwrap(r: Result<i32, ()>) -> i32 {
    r.unwrap()
}

// BAD EXPLICIT004 — `expect`
//~v EXPLICIT004_NO_PANIC
fn use_expect(r: Result<i32, ()>) -> i32 {
    r.expect("expected")
}

// BAD EXPLICIT004 — `panic!` lowers to `::std::rt::begin_panic`.
//~v EXPLICIT004_NO_PANIC
fn use_panic() {
    panic!("done");
}

// BAD EXPLICIT004 — `unreachable!()` lowers to `::core::panicking::panic`.
//~v EXPLICIT004_NO_PANIC
fn use_unreachable() -> i32 {
    unreachable!()
}

// BAD EXPLICIT004 — `todo!`
//~v EXPLICIT004_NO_PANIC
fn use_todo() -> i32 {
    todo!()
}

// BAD EXPLICIT004 — `unimplemented!`
//~v EXPLICIT004_NO_PANIC
fn use_unimplemented() -> i32 {
    unimplemented!()
}

// GOOD — `?` propagates the error and the signature carries the shape.
fn propagate(r: Result<i32, ()>) -> Result<i32, ()> {
    let _ = r?;
    Ok(0)
}

// GOOD — a typed error-returning function makes the failure explicit.
fn reads_file() -> Result<String, std::io::Error> {
    Err(std::io::Error::other("read"))
}

fn main() {}