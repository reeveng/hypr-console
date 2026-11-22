#![allow(dead_code, arithmetic_overflow, unconditional_panic)]

fn divides(value: i32) -> i32 { value / 2 }

fn references(value: &u32) -> u32 { value % 3 }

fn shifts(value: i32) -> i32 { value >> 3 }

fn masks_shift(value: u32, shift: u32) -> u32 { value << (shift & 31) }

fn narrows(value: u8) -> u32 { (value as u32) + 1 }

fn masks_add(value: u32) -> u32 { (value & 15) + 1 }

fn nonzero_divisor(value: u32, divisor: u32) -> u32 { value / (divisor | 1) }

fn negative_mask(value: i32) -> i32 { -(value & 7) }

fn evaluates_both(value: u32, effect: fn() -> u32) -> u32 { value * (effect() & 0) }

fn unknown_overflow(value: u32) -> u32 { value + 1 }

fn underflow(value: u32) -> u32 { (value & 7) - 1 }

fn signed_division_overflow(value: i32) -> i32 { value / -1 }

fn zero_divisor(value: u32) -> u32 { value / 0 }

fn invalid_shift(value: u8) -> u8 { value << 8 }

fn negative_shift(value: i32) -> i32 { value >> -1 }

fn unknown_negation(value: i32) -> i32 { -value }

fn compound(value: &mut u32, effect: fn() -> u32) { *value += effect(); }

fn effectful_shift(value: u32, effect: fn() -> u32) -> u32 { value << { effect(); 3 } }

fn named_bounds(value: u32) -> u32 { value.wrapping_mul(0) + 1 }

fn nested_effects(left: fn() -> u32, right: fn() -> u32) -> u32 { left() + right() * 0 }

fn main() {}
