// run-rustfix
// edition:2024

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
enum Mode {
    #[default]
    Standard,
    Custom,
}

fn mode(input: &Option<Mode>) -> Mode {
    input.unwrap_or_default()
}

mod shared {
    mod hidden {
        #[derive(Debug, PartialEq)]
        pub enum Order {
            Forward,
            Reverse,
        }

        impl Default for Order {
            fn default() -> Self {
                Self::Forward
            }
        }
    }

    pub use self::hidden::Order;
}

pub use self::shared::Order;

fn order(input: Option<Order>) -> Order {
    input.unwrap_or_default()
}

struct Observe {
    events: Rc<RefCell<Vec<&'static str>>>,
    name: &'static str,
}

impl Drop for Observe {
    fn drop(&mut self) {
        self.events.borrow_mut().push(self.name);
    }
}

fn select(input: Option<Observe>, fallback: Observe) -> Observe {
    input.unwrap_or(fallback)
}

fn populated(events: &Rc<RefCell<Vec<&'static str>>>) -> Option<Observe> {
    events.borrow_mut().push("receiver");
    Some(Observe { events: Rc::clone(events), name: "selected" })
}

fn alternate(events: &Rc<RefCell<Vec<&'static str>>>) -> Observe {
    events.borrow_mut().push("argument");
    Observe { events: Rc::clone(events), name: "fallback" }
}

fn taken(_: Observe, before_return: usize) {
    assert_eq!(before_return, 3);
}

fn event_count(events: &Rc<RefCell<Vec<&'static str>>>) -> usize {
    events.borrow().len()
}

fn main() {
    assert_eq!(mode(&None), Mode::Standard);
    assert_eq!(mode(&Some(Mode::Custom)), Mode::Custom);
    assert_eq!(order(None), Order::Forward);
    assert_eq!(order(Some(Order::Reverse)), Order::Reverse);

    let events = Rc::new(RefCell::new(Vec::new()));
    taken(
        populated(&events).unwrap_or(alternate(&events)),
        event_count(&events),
    );
    assert_eq!(*events.borrow(), ["receiver", "argument", "fallback", "selected"]);

    let fallback = alternate(&events);
    let selected = select(None, fallback);
    assert_eq!(selected.name, "fallback");
}
