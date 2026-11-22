// edition:2024

extern crate self as core;

fn classify(input: Option<u8>) -> u8 {
    if input.is_some() { 1 } else { 0 }
}

fn main() {
    assert_eq!(classify(Some(0)), 1);
}
