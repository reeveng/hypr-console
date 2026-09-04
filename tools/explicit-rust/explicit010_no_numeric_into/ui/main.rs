// UI test for EXPLICIT010 — a number must not change width quietly.

// BAD EXPLICIT010 — `into()` reads as the same number and is a wider one.
fn widens(small: u8) -> u32 {
    //~v EXPLICIT010_NO_NUMERIC_INTO
    small.into()
}

// GOOD — both widths are written where the reader can see them.
fn says_both(small: u8) -> u32 {
    u32::from(small)
}

// GOOD — not a number at either end.
fn a_string(text: &str) -> String {
    text.into()
}

fn main() {}
