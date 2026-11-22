// run-rustfix
// edition:2024

#[derive(Clone, Copy)]
enum Shortcut {
    Available,
    Unavailable,
}

fn thresholds(enabled: bool) -> (i32, u16) {
    (i32::from(enabled) - 1, u16::from(enabled))
}

fn main() {
    assert_eq!(thresholds(true), (0, 1));
    assert_eq!(thresholds(false), (-1, 0));
}
