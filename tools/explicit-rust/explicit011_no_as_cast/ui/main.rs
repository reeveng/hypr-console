// UI test for EXPLICIT011 — `as` casts are forbidden.

// BAD EXPLICIT011
//~v EXPLICIT011_NO_AS_CAST
fn cast(x: i64) -> i32 {
    x as i32
}

fn main() {}