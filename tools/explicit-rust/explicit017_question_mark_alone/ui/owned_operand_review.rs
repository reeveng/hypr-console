// edition:2024

use std::cell::Cell;

struct Resource<'a>(&'a Cell<u8>);

impl Drop for Resource<'_> {
    fn drop(&mut self) {
        self.0.set(1);
    }
}

fn read(_: &Resource<'_>) -> Option<String> {
    Some(String::from("value"))
}

fn evaluate(trace: &Cell<u8>) -> Option<(String, u8)> {
    Some((read(&Resource(trace))?, trace.get()))
}

fn main() {
    let trace = Cell::new(0);
    assert_eq!(evaluate(&trace), Some((String::from("value"), 0)));
    assert_eq!(trace.get(), 1);
}
