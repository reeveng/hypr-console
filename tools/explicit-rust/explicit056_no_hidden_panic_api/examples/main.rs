use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

fn taken_out(held: Vec<u8>, at: usize) -> Option<u8> {
    let mut held = held;

    //~v WARN EXPLICIT056_NO_HIDDEN_PANIC_API
    held.remove(at);

    held.first().copied()
}

fn put_in(held: Vec<u8>, at: usize, one: u8) -> Vec<u8> {
    let mut held = held;

    //~v WARN EXPLICIT056_NO_HIDDEN_PANIC_API
    held.insert(at, one);

    held
}

fn traded(held: Vec<u8>, from: usize, to: usize) -> Vec<u8> {
    let mut held = held;

    //~v WARN EXPLICIT056_NO_HIDDEN_PANIC_API
    held.swap(from, to);

    held
}

fn halved(held: Vec<u8>, at: usize) -> (Vec<u8>, Vec<u8>) {
    //~v WARN EXPLICIT056_NO_HIDDEN_PANIC_API
    match held.split_at(at) {
        (before, after) => (before.to_vec(), after.to_vec()),
    }
}

fn copied_over(into: &mut [u8], from: &[u8]) {
    //~v WARN EXPLICIT056_NO_HIDDEN_PANIC_API
    into.copy_from_slice(from);
}

fn chunked(held: &[u8]) -> Vec<&[u8]> {
    //~v WARN EXPLICIT056_NO_HIDDEN_PANIC_API
    held.chunks(0).collect()
}

fn stepped(held: &[u8]) -> Vec<u8> {
    //~v WARN EXPLICIT056_NO_HIDDEN_PANIC_API
    held.iter().step_by(0).copied().collect()
}

fn held_inside(cell: RefCell<u8>) -> Option<u8> {
    //~v WARN EXPLICIT056_NO_HIDDEN_PANIC_API
    let _ = cell.borrow_mut();

    cell.into_inner().into()
}

fn stretched(from: Instant) -> Option<Instant> {
    //~v WARN EXPLICIT056_NO_HIDDEN_PANIC_API
    let _ = from + Duration::from_secs(1);

    from.checked_add(Duration::from_secs(1))
}

fn held_shared(cell: Rc<RefCell<u8>>) -> u8 {
    //~v WARN EXPLICIT056_NO_HIDDEN_PANIC_API
    let _ = cell.borrow_mut();

    cell.try_borrow().map_or(0, |held| *held)
}

fn between(from: Instant, to: Instant) -> Duration {
    to - from
}

fn carried(held: Vec<u8>) -> Vec<u8> {
    let mut kept: Vec<u8> = Vec::new();

    for one in held.iter().step_by(2) {
        kept.push(*one);
    }

    kept
}

fn main() {
    let _ = (
        taken_out(vec![1, 2, 3], 1),
        put_in(vec![1, 2], 1, 3),
        traded(vec![1, 2], 0, 1),
        halved(vec![1, 2, 3], 1),
        copied_over(&mut [0, 0], &[1, 2]),
        chunked(&[1, 2, 3]),
        stepped(&[1, 2, 3]),
        held_inside(RefCell::new(1)),
        stretched(Instant::now()),
        held_shared(Rc::new(RefCell::new(1))),
        between(Instant::now(), Instant::now()),
        carried(vec![1, 2, 3, 4]),
    );
}
