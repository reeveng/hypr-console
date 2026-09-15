// UI test for EXPLICIT026 — the environment answered in the crate that knows
// what the answer means, and nowhere else.

// BAD EXPLICIT026 — one name, read where nothing knows what an absent one
// would mean.
fn home() -> Option<String> {
    //~v EXPLICIT026_ENV_READ_ONCE
    std::env::var("HOME").ok()
}

// BAD EXPLICIT026 — the same through a `use`, which the resolution sees
// through.
use std::env::var_os;

fn host() -> Option<std::ffi::OsString> {
    //~v EXPLICIT026_ENV_READ_ONCE
    var_os("CONSOLE_HOST")
}

// BAD EXPLICIT026 — every name at once, in a place that owns none of them.
fn how_many() -> usize {
    //~v EXPLICIT026_ENV_READ_ONCE
    std::env::vars().count()
}

// GOOD — a name handed in rather than reached for.
fn under(home: &str, name: &str) -> String {
    format!("{home}/{name}")
}

// GOOD — read at compile time, from this tree rather than from the machine.
fn named() -> &'static str {
    env!("CARGO_PKG_NAME")
}

fn main() {
    let _ = home();
    let _ = host();
    let _ = how_many();
    let _ = under("/home/somebody", "warm");
    let _ = named();
}
