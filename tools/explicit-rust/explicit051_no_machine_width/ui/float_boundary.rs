// run-rustfix
// edition:2024
// aux-build:console_core_number_conversion.rs

extern crate console_core_number_conversion;

fn at(value: f32, bytes: &[u8]) -> Option<u8> {
    let index = value as usize;
    bytes.get(index).copied()
}

fn main() {
    assert_eq!(at(1.5, &[7, 11]), Some(11));
}
