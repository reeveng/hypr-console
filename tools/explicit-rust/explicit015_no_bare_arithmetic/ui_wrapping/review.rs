#![allow(dead_code)]

fn quotient(numerator: i32, denominator: i32) -> i32 {
    numerator / denominator
}

#[repr(packed)]
struct Packed(u32);

fn packed(value: &mut Packed, amount: u32) { value.0 += amount; }
unsafe fn pointer(value: *mut u32, amount: u32) { unsafe { *value += amount; } }
fn overloaded(value: &mut u32, amount: &u32) { *value += amount; }

macro_rules! amount { () => {{ let result = 2; result }}; }
fn expanded(mut value: u32) -> u32 { value += amount!(); value }
fn expanded_binary(value: u32) -> u32 { amount!() + value }

fn main() {}
