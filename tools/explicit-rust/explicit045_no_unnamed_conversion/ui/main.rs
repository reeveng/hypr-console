// UI test for EXPLICIT045 — a conversion names the type it becomes.

use std::path::PathBuf;

// BAD EXPLICIT045 — the line says neither end of this.
fn owns(text: &str) -> String {
    //~v EXPLICIT045_NO_UNNAMED_CONVERSION
    text.into()
}

// BAD EXPLICIT045 — and the destination here is a type with a shape.
fn a_path(text: &str) -> PathBuf {
    //~v EXPLICIT045_NO_UNNAMED_CONVERSION
    text.into()
}

// GOOD — the destination is on the line and moves when the type does.
fn says_it(text: &str) -> String {
    String::from(text)
}

// GOOD — two numbers are EXPLICIT010's, which can name both widths.
fn widens(small: u8) -> u32 {
    small.into()
}

// GOOD — nothing is converted.
fn the_same(text: String) -> String {
    text
}

fn main() {}
