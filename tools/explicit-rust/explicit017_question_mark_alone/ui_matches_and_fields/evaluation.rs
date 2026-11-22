// run-rustfix
// edition:2024
use std::cell::Cell;

#[derive(Clone, Copy)]
enum Bit {
    Zero,
    One,
}

fn checked_flag(enabled: bool, read: impl FnOnce() -> Option<Bit>) -> Option<bool> {
    Some(enabled && matches!(read()?, Bit::One))
}

fn checked_guard(read: impl FnOnce() -> Option<Option<u8>>) -> Option<bool> {
    Some(core::matches![read()?, Some(byte) if byte > 3])
}

#[derive(Debug, PartialEq)]
struct Pair {
    first: u8,
    second: u8,
}

fn fields(calls: &Cell<u8>, read: impl FnOnce() -> Option<u8>) -> Option<Pair> {
    Some(Pair {
        first: { calls.set(1); 3 },
        second: read()?,
    })
}

fn checked_write(bytes: &mut [u8], trace: &Cell<u8>) -> Result<(), &'static str> {
    *(<[_]>::get_mut(&mut *bytes, {
        trace.set(trace.get() * 10 + 2);
        0
    }).ok_or("empty")?) = {
        trace.set(trace.get() * 10 + 1);
        8
    };
    Ok(())
}

fn checked_read(bytes: &[u8]) -> Option<u8> {
    Some(*(bytes.get(0)?))
}

fn checked_owned(index: usize) -> Option<[u8; 2]> {
    let mut bytes = [0, 1];
    *bytes.get_mut(index)? = 8;
    Some(bytes)
}

fn main() {
    assert_eq!(checked_flag(false, || panic!()), Some(false));
    assert_eq!(checked_flag(true, || Some(Bit::Zero)), Some(false));
    assert_eq!(checked_flag(true, || Some(Bit::One)), Some(true));
    assert_eq!(checked_flag(true, || None), None);
    assert_eq!(checked_guard(|| Some(Some(4))), Some(true));
    let calls = Cell::new(0);
    let pair = fields(&calls, || Some(calls.get()));
    assert_eq!(pair, Some(Pair { first: 3, second: 1 }));

    let mut bytes = [0];
    calls.set(0);
    assert_eq!(checked_write(&mut bytes, &calls), Ok(()));
    assert_eq!((bytes, calls.get()), ([8], 12));
    calls.set(0);
    assert_eq!(checked_write(&mut [], &calls), Err("empty"));
    assert_eq!(calls.get(), 12);
    assert_eq!(checked_read(&bytes), Some(8));
    assert_eq!(checked_read(&[]), None);
    assert_eq!(checked_owned(1), Some([0, 8]));
    assert_eq!(checked_owned(2), None);
}
