// run-rustfix
// edition:2024

trait Read { fn read(&self) -> u8; }
impl Read for u8 { fn read(&self) -> u8 { *self } }

struct Byte(u8);
impl Read for Byte { fn read(&self) -> u8 { self.0 } }
impl Byte {
    fn read(&self) -> u8 { 0 }
    fn trait_value(&self) -> u8 { (self as &dyn Read).read() }
}

fn erased(n: &u8) -> &dyn Read { n as &dyn Read }
const fn slice(data: &[u8; 2]) -> &[u8] { data as &[u8] }

fn main() {
    assert_eq!(Byte(7).read(), 0);
    assert_eq!(Byte(7).trait_value(), 7);
    assert_eq!(erased(&9).read(), 9);
    assert_eq!(slice(&[1, 2]), &[1, 2]);
}
