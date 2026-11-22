// run-rustfix
// edition:2024

macro_rules! twice {
    ($argument:expr) => { ($argument, $argument) };
}

macro_rules! spelling {
    ($argument:ident) => { stringify!($argument) };
}

fn reads_value(value: u8) -> (u8, u8) {
    let value = value + 1;
    twice!(value)
}

fn earlier_argument(value: u8) -> (u8, &'static str) {
    let name = spelling!(value);
    let value = value + 1;
    (value, name)
}

fn longer_identifier(value: u8, value_limit: u8) -> (u8, &'static str) {
    let value = value + value_limit;
    (value, spelling!(value_limit))
}

fn main() {
    assert_eq!(reads_value(4), (5, 5));
    assert_eq!(earlier_argument(4), (5, "value"));
    assert_eq!(longer_identifier(4, 2), (6, "value_limit"));
}
