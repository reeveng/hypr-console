// run-rustfix
// edition:2024
#![allow(dead_code)]

fn apply(mut offset: i32, delta: i32, shift: i32) -> (i32, i32) {
    offset += 9;
    let sum = offset + delta;
    let shifted = sum << shift;
    let reflected = -shifted;
    (offset, reflected)
}

fn advance(start: usize, count: usize) -> usize {
    start + count
}

fn main() {}
