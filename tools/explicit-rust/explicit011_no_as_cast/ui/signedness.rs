// run-rustfix
// edition:2024
// rustc-env:CARGO_PKG_RUST_VERSION=1.87
#![allow(dead_code)]

const fn bits(value: i32) -> u32 { value as u32 }
fn signed(read: fn() -> u8) -> i8 { read() as i8 }
fn offset(value: usize) -> isize { value as isize }

fn main() {
    assert_eq!(bits(-1), u32::MAX);
    assert_eq!(signed(|| 128), i8::MIN);
}
