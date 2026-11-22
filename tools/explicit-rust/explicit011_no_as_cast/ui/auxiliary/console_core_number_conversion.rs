#![expect(explicit011_no_as_cast, reason = "primitive casts stand in for the shared conversion functions in this fixture")]

use std::convert::Infallible;

extern crate self as console_core_never;

pub type Never = Infallible;
#[path = "../../../../../crates/console-core-number-conversion/src/const_integer_bits.rs"]
mod const_integer_bits;
pub use const_integer_bits::{resize_signed_le_bytes, resize_unsigned_le_bytes};

pub trait LowBits {
    fn from_low_bits(bits: i128) -> Self;
}

impl LowBits for u8 {
    fn from_low_bits(bits: i128) -> Self {
        bits.to_le_bytes()[0]
    }
}

impl LowBits for i16 {
    fn from_low_bits(bits: i128) -> Self {
        let [first, second, ..] = bits.to_le_bytes();
        i16::from_le_bytes([first, second])
    }
}

pub fn wrapped<F: Into<i128>, T: LowBits>(value: F) -> Result<T, Infallible> {
    Ok(T::from_low_bits(value.into()))
}

pub fn nearest_f32_from_f64(value: f64) -> Result<f32, Infallible> {
    Ok(value as f32)
}

pub fn nearest_f32_from_i64(value: i64) -> Result<f32, Infallible> {
    Ok(value as f32)
}

pub fn nearest_f32_from_u64(value: u64) -> Result<f32, Infallible> {
    Ok(value as f32)
}

pub fn toward_zero_u8(value: f64) -> Result<u8, Infallible> {
    Ok(value as u8)
}
