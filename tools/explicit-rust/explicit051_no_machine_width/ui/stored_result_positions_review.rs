// edition:2024
// aux-build:console_core_number_conversion.rs

extern crate console_core_number_conversion;

use std::convert::Infallible;

#[allow(explicit051_no_machine_width)]
type Position = usize;

fn find_position(found: bool) -> Result<Position, ()> {
    if found { Ok(1) } else { Err(()) }
}

fn read(data: &[u8], found: bool) -> u8 {
    let Ok(index) = find_position(found) else { return 0; };
    data[index]
}

fn position() -> Result<Position, Infallible> { Ok(1) }

fn address() -> u8 {
    let Ok(index) = position();
    use_address(&index)
}

fn use_address(index: &Position) -> u8 { *index as u8 }

mod imported {
    #[allow(non_upper_case_globals)]
    pub const native_position: u8 = 4;
}

fn glob(data: &[u8]) -> u8 {
    use imported::*;
    let Ok(index) = position();
    data[index] + native_position
}

fn main() {
    assert_eq!(read(&[4, 9], true), 9);
    assert_eq!(address(), 1);
    assert_eq!(glob(&[4, 9]), 13);
}
