// edition:2024
// aux-build:console_core_number_conversion.rs

extern crate console_core_number_conversion;

#[allow(explicit051_no_machine_width)]
type Position = usize;

const fn const_shift(distance: u32) -> Position {
    usize::wrapping_shl(1, distance)
}

fn oversized(distance: u32) -> Position {
    usize::wrapping_shr(70_000, distance)
}

fn main() {
    assert_eq!(const_shift(2), 4);
    assert_eq!(oversized(1), 35_000);
}
