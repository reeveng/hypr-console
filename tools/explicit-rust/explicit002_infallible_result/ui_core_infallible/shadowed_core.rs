// edition:2024

extern crate self as core;

fn trimmed_length(text: &str) -> usize { text.trim().len() }

fn main() {
    let length = trimmed_length(" label ");
    assert_eq!(length, 5);
}
