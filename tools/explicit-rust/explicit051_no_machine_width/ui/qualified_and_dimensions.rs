// run-rustfix
// edition:2024
// aux-build:console_core_number_conversion.rs
extern crate console_core_number_conversion;

#[allow(explicit051_no_machine_width)]
type Position = usize;
#[allow(explicit051_no_machine_width)]
type Offset = isize;

trait AnotherFrom {
    fn from(_: bool) -> Self;
}
impl AnotherFrom for u8 {
    fn from(_: bool) -> Self { 99 }
}

fn direction(directions: &[i8; 2], at: Position) -> Offset {
    let off = <isize as ::core::convert::From<::core::primitive::i8>>::from(directions[at]);
    off
}

fn disabled(value: bool) -> Position {
    <usize as From<bool>>::from(value)
}

fn plane_len() -> Position {
    usize::wrapping_mul(64, 71)
}

fn buffers() -> Vec<u8> {
    vec![0; usize::wrapping_mul(64, 64)]
}

fn padding() -> Position {
    usize::wrapping_sub(71, 3)
}

fn main() {
    assert_eq!(direction(&[-128, 127], 0), -128);
    assert_eq!(disabled(true), 1);
    assert_eq!(plane_len(), 4544);
    assert_eq!(buffers().len(), 4096);
    assert_eq!(padding(), 68);
    assert_eq!(<u8 as AnotherFrom>::from(true), 99);
}
