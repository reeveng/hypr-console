#![allow(dead_code, arithmetic_overflow, unconditional_panic)]

fn may_overflow(a: u16, b: u16) -> i32 { i32::from(a) * i32::from(b) }
fn zero(x: &mut i32) { *x /= 0; }
fn negative(mut x: i32) -> i32 { x /= -1; x }
fn wide_shift(mut x: u8) -> u8 { x <<= 8; x }
fn evaluates_place(a: &mut [u32], i: fn() -> usize) { a[i()] >>= 1; }
fn evaluates_rhs(mut x: u32, f: fn() -> u32) -> u32 { x %= f(); x }
fn commented(mut x: u32) -> u32 { x /* keep */ /= 2; x }

fn large_remainder(value: u32, n: u32) -> u32 { value << n.wrapping_rem(33) }
fn signed_remainder(value: u32, n: i32) -> u32 { value << n.wrapping_rem(32) }
fn nested(mut value: u32) -> ((), u32) { (value /= 2, value) }
fn main() {}
