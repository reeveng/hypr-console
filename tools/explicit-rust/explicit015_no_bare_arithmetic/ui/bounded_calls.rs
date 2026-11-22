// run-rustfix
// edition:2024
#![allow(unused_parens)]

use std::num::NonZeroU32;

fn bounded(x: u8, shift: i32, divisor: NonZeroU32) -> (u8, u32, u32, i32) {
    let limited = x.min(14) + 1;
    let forced = 64 / core::cmp::max(u32::from(x), 2);
    let nonzero = 64 % divisor.get();
    let shifted = 128 >> Ord::clamp(shift, 0, 6);
    (limited, forced, nonzero, shifted)
}

fn compound(mut x: u32) -> u32 {
    x += 0;
    x -= 0;
    x *= 1;
    x
}

fn main() {
    let Some(divisor) = NonZeroU32::new(3) else { panic!() };
    assert_eq!(bounded(16, 8, divisor), (15, 4, 1, 2));
    assert_eq!(compound(u32::MAX), u32::MAX);
}
