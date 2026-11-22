// run-rustfix
// edition:2024

use std::cell::RefCell;

struct Step<'a>(&'a RefCell<Vec<u8>>);

impl Drop for Step<'_> {
    fn drop(&mut self) {
        self.0.borrow_mut().push(3);
    }
}

fn amount(step: &Step<'_>) -> u32 {
    step.0.borrow_mut().push(1);
    3
}

fn index(trace: &RefCell<Vec<u8>>) -> usize {
    trace.borrow_mut().push(2);
    0
}

fn main() {
    let trace = RefCell::new(Vec::new());
    let mut values = [7_u32];
    values[index(&trace)] += amount(&Step(&trace));
    assert_eq!(values, [10]);
    assert_eq!(trace.into_inner(), [1, 2, 3]);

    let mut scalar = 2_u32;
    assert_eq!(scalar, 2);
    scalar -= {
        scalar = 8;
        1
    };
    assert_eq!(scalar, 7);
}
