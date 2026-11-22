// run-rustfix
// edition:2024

use std::cell::RefCell;
use std::rc::Rc;

type Trace = Rc<RefCell<Vec<u8>>>;

struct Resource(Trace);

impl Drop for Resource {
    fn drop(&mut self) {
        self.0.borrow_mut().push(4);
    }
}

fn observe(trace: &Trace, event: u8) -> u8 {
    trace.borrow_mut().push(event);
    event
}

fn load(trace: &Trace, valid: bool) -> Option<Option<Resource>> {
    observe(trace, 2);
    valid.then(|| Some(Resource(Rc::clone(trace))))
}

fn tuple(trace: &Trace, valid: bool) -> Option<(u8, Option<Resource>)> {
    Some((observe(trace, 1), load(trace, valid)?))
}

fn matched(trace: &Trace, valid: bool) -> Option<()> {
    match load(trace, valid)? {
        Some(value) => {
            observe(trace, 3);
            drop(value);
        }
        None => return None,
    }
    Some(())
}

fn assigned(trace: &Trace, should_load: bool) -> Option<()> {
    let mut value = None;
    match should_load {
        true => value = load(trace, true)?,
        false => {}
    }
    observe(trace, 3);
    drop(value);
    Some(())
}

fn main() {
    let trace: Trace = Default::default();
    let value = tuple(&trace, true);
    assert_eq!(*trace.borrow(), [1, 2]);
    drop(value);
    assert_eq!(*trace.borrow(), [1, 2, 4]);
    trace.borrow_mut().clear();
    assert!(tuple(&trace, false).is_none());
    assert_eq!(*trace.borrow(), [1, 2]);
    trace.borrow_mut().clear();
    assert_eq!(matched(&trace, true), Some(()));
    assert_eq!(*trace.borrow(), [2, 3, 4]);
    trace.borrow_mut().clear();
    assert_eq!(matched(&trace, false), None);
    assert_eq!(*trace.borrow(), [2]);
    trace.borrow_mut().clear();
    assert_eq!(assigned(&trace, true), Some(()));
    assert_eq!(*trace.borrow(), [2, 3, 4]);
}
