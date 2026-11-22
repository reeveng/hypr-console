#![allow(dead_code)]

fn temporary(make: fn() -> u8) {
    let bytes = &[make()] as &[u8];
    std::hint::black_box(bytes);
}
fn mutable(bytes: &mut [u8; 2]) -> &mut [u8] { bytes as &mut [u8] }
fn main() {}
