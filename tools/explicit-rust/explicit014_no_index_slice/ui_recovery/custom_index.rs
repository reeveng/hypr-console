// edition:2024
use std::ops::{Deref, Index, RangeFrom};
#[derive(Debug, PartialEq)]
enum Fault { Bounds }
struct Remapped([u8; 2]);
impl Deref for Remapped {
    type Target = [u8; 2];
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl Index<usize> for Remapped {
    type Output = u8;
    fn index(&self, _: usize) -> &u8 { self.0.first().unwrap() }
}
impl Index<RangeFrom<usize>> for Remapped {
    type Output = [u8];
    fn index(&self, _: RangeFrom<usize>) -> &[u8] { self.0.as_slice() }
}
fn read(values: &Remapped, index: usize) -> Result<u8, Fault> {
    Ok(values[index])
}
fn tail(values: &Remapped, start: usize) -> Result<&[u8], Fault> {
    Ok(&values[start..])
}
fn main() {
    assert_eq!(read(&Remapped([4, 9]), 123), Ok(4));
    assert_eq!(tail(&Remapped([4, 9]), 123), Ok([4, 9].as_slice()));
}
