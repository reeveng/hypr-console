// run-rustfix
// edition:2024

fn multiply(a: i16, b: i16) -> i32 { i32::from(a) * i32::from(b) }
fn subtract(a: i16, b: u16) -> i32 { a as i32 - i32::from(b) }
fn add(a: i8) -> i32 { i32::from(a) + 128 }

fn shift(value: u32, index: u32) -> u32 { value << index.wrapping_rem(32) }
fn rounded(value: u32) -> u32 { value.wrapping_div(2) + 1 }

fn scale(mut value: i32) -> i32 {
    value /= 3;
    value %= 13;
    value <<= 1_u8;
    value >>= 2;
    value
}

fn main() {
    assert_eq!(multiply(i16::MIN, i16::MIN), 1_073_741_824);
    assert_eq!(subtract(i16::MIN, u16::MAX), -98_303);
    assert_eq!(add(i8::MIN), 0);
    assert_eq!(scale(-30), -5);
    assert_eq!(shift(1, 63), 2_147_483_648);
    assert_eq!(rounded(u32::MAX), 2_147_483_648);
}
