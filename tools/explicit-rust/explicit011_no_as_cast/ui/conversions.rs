// run-rustfix
// edition:2024
#![allow(dead_code)]

fn widen(read: fn() -> u16) -> u64 {
    read() as u64
}

fn byte_code(value: u8) -> char {
    value as char
}

type Sample = u32;

fn sample(value: u8) -> Sample {
    value as Sample
}

fn main() {}
