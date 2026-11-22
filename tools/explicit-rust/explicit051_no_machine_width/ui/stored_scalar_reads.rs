// run-rustfix
// edition:2024
// aux-build:console_core_number_conversion.rs
extern crate console_core_number_conversion;

fn pair<T>(value: T) -> (T, T) where T: Copy { (value, value) }

fn measured(bytes: &[u8], edge: &[u8]) -> (u64, bool, u64, u64) {
    let position = bytes.len();
    (position as u64, position < edge.len(), pair(position).0 as u64,
     position.wrapping_add(!0) as u64)
}

fn indirect(bytes: &[u8]) -> Option<u64> {
    let count = bytes.len();
    Some(count).map(|n| n as u64)
}

fn through_function_value(bytes: &[u8]) -> u64 {
    let identity = core::convert::identity;
    let count = bytes.len();
    identity(count) as u64
}

fn main() {
    assert_eq!(measured(&[1, 2], &[3, 4, 5]), (2, true, 2, 1));
    assert_eq!(indirect(&[]), Some(0));
    assert_eq!(through_function_value(&[1]), 1);
}
