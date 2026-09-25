// UI test for EXPLICIT051 — a number has a width the source says.

use std::convert::TryFrom;
use std::num::NonZeroUsize;

// BAD EXPLICIT051 — a field, whatever it is for.
struct Tabs {
    //~v EXPLICIT051_NO_MACHINE_WIDTH
    at: usize,
    //~v EXPLICIT051_NO_MACHINE_WIDTH
    many: NonZeroUsize,
}

// BAD EXPLICIT051 — a signature, both ends of it.
//~v EXPLICIT051_NO_MACHINE_WIDTH
fn moved(by: isize) -> u32 {
    let _ = by;
    0
}

// BAD EXPLICIT051 — a binding and a list that say the machine's width.
fn held() -> u32 {
    //~v EXPLICIT051_NO_MACHINE_WIDTH
    let many: Vec<usize> = Vec::new();
    let _ = many;
    0
}

// BAD EXPLICIT051 — a binding the compiler typed holds the width all the same.
fn inferred(items: &[u32]) -> u32 {
    //~v EXPLICIT051_NO_MACHINE_WIDTH
    let many = items.len();
    //~v EXPLICIT051_NO_MACHINE_WIDTH
    let found = items.iter().position(|item| *item == 3);
    let _ = (many, found);
    0
}

// GOOD — an array's length is not a number anybody holds.
fn listed() -> u32 {
    let names = ["one", "two"];
    let bytes = b"four";
    let _counted_and_let_go = names.len();
    let _ = (names, bytes);
    0
}

// GOOD — the width is said, and the `usize` a library hands over is never
// written down.
fn counted(items: &[u32]) -> u64 {
    let many = u64::try_from(items.len()).unwrap_or(u64::MAX);
    match items.get(0) {
        Some(_) => many,
        None => 0,
    }
}

fn main() {
    //~v EXPLICIT051_NO_MACHINE_WIDTH
    let tabs = Tabs { at: 0, many: NonZeroUsize::MIN };
    let _ = (tabs.at, tabs.many, moved(0), held(), inferred(&[]), listed(), counted(&[]));
}
