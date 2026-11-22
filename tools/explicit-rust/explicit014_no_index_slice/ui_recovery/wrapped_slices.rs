// run-rustfix
// edition:2024
use std::ops::{Deref, DerefMut};
use std::cell::Cell;

#[derive(Debug, PartialEq)]
enum Fault { Bounds }
#[repr(align(8))]
struct Aligned<T>(T);
impl<T> Deref for Aligned<T> {
    type Target = T;
    fn deref(&self) -> &T { &self.0 }
}
impl<T> DerefMut for Aligned<T> {
    fn deref_mut(&mut self) -> &mut T { &mut self.0 }
}
struct Block(Aligned<[u8; 16]>);
impl Block {
    fn get_mut(&mut self) -> &mut Aligned<[u8; 16]> { &mut self.0 }
}
fn context(input: &Aligned<[u8; 16]>, i: usize) -> Result<u8, Fault> {
    Ok(input[i])
}
fn set_context(block: &mut Block, start: usize, width: usize) -> Result<(), Fault> {
    let tail = &mut block.get_mut()[start..];
    tail[..width].fill(7);
    Ok(())
}
fn fixed(table: &mut Aligned<[[u8; 4]; 2]>) {
    table[1].fill(11);
}
struct Traced<'a>([u8; 4], &'a Cell<u8>);
impl Deref for Traced<'_> {
    type Target = [u8; 4];
    fn deref(&self) -> &Self::Target { self.1.set(self.1.get() * 10 + 1); &self.0 }
}
fn read_in_order(values: &Traced<'_>, trace: &Cell<u8>) -> Result<u8, Fault> {
    Ok(values[{ trace.set(trace.get() * 10 + 2); 1 }])
}
fn main() {
    let mut block = Block(Aligned([1; 16]));
    assert_eq!(context(&block.0, 3), Ok(1));
    assert_eq!(set_context(&mut block, 14, 2), Ok(()));
    assert_eq!(context(&block.0, 15), Ok(7));
    let mut table = Aligned([[1; 4]; 2]);
    fixed(&mut table);
    assert_eq!(table.0, [[1; 4], [11; 4]]);
    let trace = Cell::new(0);
    assert_eq!(read_in_order(&Traced([3; 4], &trace), &trace), Ok(3));
    assert_eq!(trace.get(), 12);
    if std::env::args().any(|arg| arg == "--failure") {
        assert_eq!(context(&block.0, 16), Err(Fault::Bounds));
        assert_eq!(set_context(&mut block, 17, 0), Err(Fault::Bounds));
        assert_eq!(set_context(&mut block, 15, 2), Err(Fault::Bounds));
        assert_eq!(context(&block.0, 15), Ok(7));
    }
}
