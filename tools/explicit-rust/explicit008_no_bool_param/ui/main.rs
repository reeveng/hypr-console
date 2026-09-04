// UI test for EXPLICIT008 — bool parameters are forbidden.

// BAD EXPLICIT008
//~v EXPLICIT008_NO_BOOL_PARAM
fn write_count(n: usize, append: bool) -> usize {
    if append { n } else { 0 }
}

// GOOD — an enum carries the choice legibly.
#[derive(Debug, Clone, Copy)]
enum Mode { Append, Truncate }
fn write_count_ok(n: usize, mode: Mode) -> usize {
    if matches!(mode, Mode::Append) { n } else { 0 }
}

// GOOD — `#[test]` is exempt.
#[test]
fn check(b: bool) -> bool { b }

// GOOD -- the trait fixed this signature, so the impl had no choice to make.
trait Switch {
    fn flip(&self, on: bool);
}
struct Lamp;
impl Switch for Lamp {
    fn flip(&self, on: bool) {
        let _ = on;
    }
}

fn main() {}