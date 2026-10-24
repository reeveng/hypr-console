use std::rc::Rc;
use std::sync::{Arc, Mutex, Weak};
use std::thread;

mod mine {
    pub struct Arc(pub u8);
}

type Shared = Arc<Mutex<u8>>;

struct Held {
    count: Shared,
    seen: Rc<u8>,
    gone: Weak<u8>,
}

struct Owned {
    count: Mutex<u8>,
}

fn shared(count: &Shared) -> Shared {
    count.clone()
}

fn lent(owned: &Owned) -> u8 {
    thread::scope(|scope| scope.spawn(|| owned.count.lock().map_or(0, |held| *held)).join().map_or(0, |held| held))
}

fn main() {
    let count: Shared = Arc::new(Mutex::new(1));
    let again = shared(&count);
    let gone = Arc::downgrade(&Arc::new(1));
    let held = Held { count: again, seen: Rc::new(1), gone };
    let owned = Owned { count: Mutex::new(1) };
    let named = mine::Arc(1);

    let _ = (held.count, held.seen, held.gone, lent(&owned), named.0);
}
