// edition:2024
#![allow(dead_code)]

fn shifted_out(row: u32, shift: u32) -> u32 { row / 2u32.wrapping_shl(shift) }
fn right_shift_to_zero(row: u32, shift: u32) -> u32 { row % 1u32.wrapping_shr(shift) }
fn insufficient_divisor(row: usize, shift: u32) -> usize {
    let ctb = 1usize.wrapping_shl(shift);
    row / ctb.wrapping_div(2)
}
fn signed_overflow(row: i8, shift: u32) -> i8 { row / (-1i8).wrapping_shl(shift) }
fn high_bits_remain(bits: u32, shift: u32) -> u32 { bits.wrapping_shl(shift) + 1 }
fn wide_count(bytes: u64) -> u32 { 32 - bytes.leading_zeros() }
fn zero_count(records: u32) -> u32 { 31 - records.leading_zeros() }
fn target_width(words: usize) -> u32 { 32 - words.leading_zeros() }
fn target_shift(n: usize) -> usize { 1usize.wrapping_shl(32) + n }
fn mutable_shift(block: i32, flag: bool, next: usize) -> i32 {
    let mut sx = usize::from(flag);
    if flag { sx = next; }
    block >> sx
}
struct Font;
impl Font {
    fn leading_zeros(&self) -> u32 { u32::MAX }
    fn wrapping_shl(&self, _: u32) -> u32 { 0 }
}
fn unrelated(font: &Font) -> (u32, u32) {
    (32 - font.leading_zeros(), 17 / font.wrapping_shl(1))
}
fn main() {}
