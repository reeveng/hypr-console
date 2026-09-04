// UI test for EXPLICIT013 — `if`, `match`, `while`, `for`, `loop` and function
// declarations are set off by a blank line above and below.
//
// The dylint test harness reads the diagnostics against `ui/main.stderr`.
// Lines that already read as a gap must produce nothing.

// BAD: an `if` pressed against the statement above and the one below.
fn crowded_if(n: u32) -> u32 {
    let doubled = n * 2;
    if doubled > 10 {
        return 10;
    }
    doubled
}

// GOOD: the same thing, with room around it.
fn spaced_if(n: u32) -> u32 {
    let doubled = n * 2;

    if doubled > 10 {
        return 10;
    }

    doubled
}

// GOOD: nothing above to be separated from, and the `}` below closes the fn.
fn only_statement(n: u32) {
    if n > 0 {
        println!("{n}");
    }
}

// GOOD: a comment belongs to the block it explains, so the gap goes above the
// pair rather than between them.
fn commented_if(n: u32) -> u32 {
    let doubled = n * 2;

    // Ten is as high as this counts.
    if doubled > 10 {
        return 10;
    }

    doubled
}

// GOOD: an `else` chain is one thought and takes no gaps inside it.
fn chained(n: u32) -> u32 {
    if n > 10 {
        1
    } else if n > 5 {
        2
    } else {
        3
    }
}

// GOOD: an `if` used as a value is an expression in the middle of a line.
fn assigned(wide: bool) -> u32 {
    let width = if wide { 3 } else { 1 };
    width + 1
}

// GOOD: a `match` that is the last expression of a function is not a statement.
fn matched(n: u32) -> &'static str {
    match n {
        0 => "none",
        _ => "some",
    }
}

// BAD: a `match` written as a statement, with no room around it.
fn crowded_match(n: u32) {
    let seen = n;
    match seen {
        0 => println!("none"),
        _ => println!("some"),
    }
    println!("done");
}

// BAD: a `for` pressed against what follows it.
fn crowded_for() {
    for i in 0..3 {
        println!("{i}");
    }
    println!("done");
}

// BAD: a `while` pressed against what comes before it.
fn crowded_while() {
    let mut n = 0;
    while n < 3 {
        n += 1;
    }
}

// BAD: a function declaration with no blank line above it.
fn first() {}
fn second() {}

fn main() {}

// BAD: a `let … else` is a branch wearing a `let`, and gets the same room.
fn crowded_let_else(n: Option<u32>) -> u32 {
    let seen = 1;
    let Some(found) = n else {
        return seen;
    };
    found
}

// GOOD: the same, with room around it.
fn spaced_let_else(n: Option<u32>) -> u32 {
    let seen = 1;

    let Some(found) = n else {
        return seen;
    };

    found + seen
}

// BAD: a struct declaration with no blank line below it.
struct Packed {
    n: u32,
}
impl Packed {
    fn n(&self) -> u32 {
        self.n
    }
}

// GOOD: `use` and `const` are written in groups and are left alone.
use std::fmt::Debug;
use std::fmt::Display;

const ONE: u32 = 1;
const TWO: u32 = 2;

fn spends(a: &dyn Debug, b: &dyn Display) -> u32 {
    let _ = (a, b);

    ONE + TWO
}

// BAD: an `unsafe` block written as a statement, pressed against what follows.
fn crowded_unsafe(p: *mut u8) {
    let n = 1;
    // SAFETY: the caller guarantees `p` points to writable memory.
    unsafe {
        *p = n;
    }
    println!("wrote");
}
