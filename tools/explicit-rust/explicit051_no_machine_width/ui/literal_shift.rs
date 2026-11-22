// run-rustfix
// edition:2024
// aux-build:console_core_number_conversion.rs

extern crate console_core_number_conversion;

#[allow(explicit051_no_machine_width)]
type Position = usize;
#[allow(explicit051_no_machine_width)]
type Offset = isize;

#[allow(non_upper_case_globals)]
const native_shift_input: Position = 7;

fn left(distance: u32, order: &mut Vec<u8>) -> Position {
    usize::wrapping_shl(0x1, { order.push(1); distance })
}

fn right(distance: u32) -> Offset {
    isize::wrapping_shr(6, distance)
}

fn main() {
    let mut order = Vec::new();
    for distance in [0, 2, Position::BITS, Position::BITS + 1, u32::MAX] {
        assert_eq!(left(distance, &mut order), Position::wrapping_shl(1, distance));
        assert_eq!(right(distance), Offset::wrapping_shr(6, distance));
    }
    assert_eq!(order, [1; 5]);
    assert_eq!(native_shift_input, 7);
}
