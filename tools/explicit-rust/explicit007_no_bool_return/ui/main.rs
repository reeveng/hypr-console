// UI test for EXPLICIT007 — bool return values are forbidden.

// BAD EXPLICIT007
//~v EXPLICIT007_NO_BOOL_RETURN
fn saves() -> bool {
    true
}

// GOOD — an enum tells the caller what went wrong.
#[derive(Debug, PartialEq, Eq)]
enum Save { Outcome { kind: u8 }, }
fn saves_ok() -> Result<Save, std::io::Error> {
    Ok(Save::Outcome { kind: 0 })
}

// GOOD — a `#[test]` function returning bool is exempt.
#[test]
fn truth() -> bool {
    true
}

// GOOD -- the trait fixed this signature, so the impl had no choice to make.
struct Pair(u8);
impl PartialEq for Pair {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

fn main() {}