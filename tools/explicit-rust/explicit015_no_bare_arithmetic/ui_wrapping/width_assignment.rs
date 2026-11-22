// run-rustfix
// edition:2024
// aux-build:console_core_number_conversion.rs
extern crate console_core_number_conversion;

use std::cell::RefCell;

struct Step<'a>(&'a RefCell<Vec<u8>>);

impl Drop for Step<'_> {
    fn drop(&mut self) {
        self.0.borrow_mut().push(3);
    }
}

fn amount(step: &Step<'_>) -> usize {
    step.0.borrow_mut().push(1);
    3
}

fn index(trace: &RefCell<Vec<u8>>) -> usize {
    trace.borrow_mut().push(2);
    0
}

fn main() {
    let trace = RefCell::new(Vec::new());
    let mut values = [7_usize];
    values[index(&trace)] += amount(&Step(&trace));
    assert_eq!(values, [10]);
    assert_eq!(trace.into_inner(), [1, 2, 3]);
}
