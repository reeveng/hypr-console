// edition:2024

macro_rules! pass {
    ($argument:expr) => { $argument };
}

macro_rules! name {
    ($argument:ident) => { (stringify!($argument), $argument) };
}

fn held(value: u8) -> (&'static str, u8) {
    let value = value + 1;
    pass!(name!(value))
}

fn assertion_text(value: u8) {
    let value = value + 1;
    assert!(value > 0);
}

fn captured_format(value: u8) -> String {
    let value = value + 1;
    format!("{value}")
}

fn main() {
    assert_eq!(held(4), ("value", 5));
    assertion_text(4);
    assert_eq!(captured_format(4), "5");
}
