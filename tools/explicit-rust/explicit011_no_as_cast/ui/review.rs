#![allow(dead_code)]

const fn in_const(value: u8) -> u32 { value as u32 }
fn signed(value: i8) -> u16 { value as u16 }
fn rounds(value: f64) -> u32 { value as u32 }
fn annotated(value: u8) -> u32 { value /* wire byte */ as u32 }
macro_rules! widen { ($v:expr) => { $v as u32 } }
fn expanded(value: u8) -> u32 { widen!(value) }

fn main() {}
