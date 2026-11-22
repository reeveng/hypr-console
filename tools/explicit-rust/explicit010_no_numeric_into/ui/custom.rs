#![no_implicit_prelude]
#![allow(dead_code)]

trait MyConversion {
    fn into(self) -> u32;
}
impl MyConversion for u8 {
    fn into(self) -> u32 { 42 }
}

fn custom(value: u8) -> u32 { value.into() }

fn main() {}
