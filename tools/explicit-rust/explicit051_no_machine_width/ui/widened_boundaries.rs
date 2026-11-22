// run-rustfix
// edition:2024
// aux-build:console_core_number_conversion.rs

extern crate console_core_number_conversion;

#[allow(explicit051_no_machine_width)]
type Position = usize;

#[allow(explicit051_no_machine_width)]
type Offset = isize;

fn pick(flag: bool, bytes: &[u8; 2]) -> u8 {
    let at = usize::from(flag);
    bytes[at]
}

fn extend(byte: u8) -> Position {
    usize::from(byte)
}

fn signed(word: i16) -> Offset {
    let mut offset = isize::from(word);
    offset = offset.wrapping_add(1);
    offset
}

fn anchored(make: bool, positions: &[Position; 2], end: Position) -> Position {
    let (_at, _) = (positions[0], 1_u8);
    let _next = match make {
        true => end,
        false => 0,
    };
    usize::wrapping_add(usize::wrapping_mul(3, _at), usize::wrapping_shl(1usize, 2))
        .wrapping_add(usize::wrapping_mul(2, _next))
}

fn main() {
    assert_eq!(pick(false, &[10, 20]), 10);
    assert_eq!(pick(true, &[10, 20]), 20);
    assert_eq!(extend(255), 255);
    assert_eq!(signed(-20), -19);
    assert_eq!(anchored(true, &[2, 3], 4), 18);
}
