// run-rustfix
// edition:2024
use std::cell::Cell;

fn short(keep: bool, read: impl FnOnce() -> Option<bool>) -> Option<bool> {
    let answer = keep && read()?;
    Some(answer)
}

fn observe(calls: &Cell<u8>) -> Option<(u8, u8)> {
    let pair = (calls.get(), { calls.set(8); Some(1) }?);
    Some(pair)
}

fn main() {
    assert_eq!(short(false, || panic!()), Some(false));
    assert_eq!(observe(&Cell::new(4)), Some((4, 1)));
}
