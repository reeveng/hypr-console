// run-rustfix
// edition:2024
use std::{cell::Cell, convert::Infallible};

struct Probe<'a>(&'a Cell<u16>);

impl Probe<'_> {
    fn stride(&self) -> Result<isize, Infallible> {
        self.0.set(self.0.get() * 10 + 2);
        Ok(-3)
    }
}

impl Drop for Probe<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() * 10 + 4);
    }
}

fn combine(_: u8, position: isize, _: u8) -> isize { position }

fn ordered(trace: &Cell<u16>) -> Result<isize, Infallible> {
    let result = combine(
        { trace.set(1); 0 },
        Probe(trace).stride()?,
        { trace.set(trace.get() * 10 + 3); 0 },
    );
    Ok(result)
}

fn index() -> Result<usize, Infallible> { Ok(1) }

fn indexed(bytes: &[u8; 2]) -> Result<u8, Infallible> {
    let next = index()?.wrapping_add(0);
    Ok(bytes[index()?] + bytes[next])
}

fn main() {
    let trace = Cell::new(0);
    assert_eq!(ordered(&trace), Ok(-3));
    assert_eq!(trace.get(), 1234);
    assert_eq!(indexed(&[2, 3]), Ok(6));
}
