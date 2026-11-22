// run-rustfix
// edition:2024
#![allow(dead_code)]

fn middle(values: &mut [u8; 8]) -> &mut [u8] {
    &mut values[2..=5]
}

fn prefix(values: &[u8; 8]) -> &[u8] {
    &values[..4]
}

fn empty(values: &[u8; 8]) -> &[u8] {
    &values[3..3]
}

fn end_of_row(values: &[String; 64]) -> &String {
    &values[62]
}

fn stored(block: &mut [[u8; 4]; 64]) {
    block[63][1] = 7;
}

fn last(values: [u8; 32]) -> u8 {
    let last = values[31];
    last
}

fn main() {
    let mut values = [0, 1, 2, 3, 4, 5, 6, 7];
    assert_eq!(prefix(&values), &[0, 1, 2, 3]);
    assert_eq!(empty(&values).as_ptr(), values.as_ptr().wrapping_add(3));
    middle(&mut values).fill(10);
    assert_eq!(values, [0, 1, 10, 10, 10, 10, 6, 7]);
}
