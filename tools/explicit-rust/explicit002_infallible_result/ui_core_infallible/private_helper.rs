// run-rustfix
// edition:2024

#![no_std]
extern crate std;

fn trimmed_length(text: &str) -> usize { text.trim().len() }

fn main() {
    let length = trimmed_length(" label ");
    std::assert_eq!(length, 5);
}
