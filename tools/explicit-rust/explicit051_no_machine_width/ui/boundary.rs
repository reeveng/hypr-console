// run-rustfix
// edition:2024
// aux-build:console_core_number_conversion.rs
extern crate console_core_number_conversion;

fn main() {
    let x = -1_i32;
    let Ok(position) = console_core_number_conversion::fitted::<_, usize>(7_u8);
    assert_eq!(x as usize, !0);
    assert_eq!(255_u8 as isize, 255);
    assert_eq!(position, 7);
}
