// edition:2024

#[derive(Clone, Copy)]
enum Shortcut {
    Available,
    Unavailable,
}

struct Custom;

impl Custom {
    fn from(value: impl Into<u8>) -> u16 {
        u16::from(value.into()) + 9
    }
}

fn threshold(enabled: bool) -> u16 {
    Custom::from(enabled)
}

fn main() {
    assert_eq!(threshold(true), 10);
}
