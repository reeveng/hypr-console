// UI test for EXPLICIT002 — registered `Allow`, so nothing here is denied by
// default. The fixture is what the rule would say if it were turned on.

fn plain() -> i32 {
    0
}

fn nothing() {}

fn already_says_so() -> Result<i32, ()> {
    Ok(0)
}

fn main() {}
