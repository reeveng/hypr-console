// run-rustfix
// edition:2024
use std::cell::Cell;

struct Bounds { prev_end: u32, end: u32 }
impl Bounds {
    fn extend(&mut self, width: u32) -> Option<()> {
        self.end = self.prev_end.checked_add(width)?;
        Some(())
    }
}
fn slot(trace: &Cell<u8>) -> usize {
    trace.set(trace.get() * 10 + 2);
    0
}
fn read(trace: &Cell<u8>, succeed: bool) -> Option<u8> {
    trace.set(trace.get() * 10 + 1);
    succeed.then_some(7)
}
fn write(bytes: &mut [u8; 1], trace: &Cell<u8>, succeed: bool) -> Option<()> {
    bytes[slot(trace)] = read(trace, succeed)?;
    Some(())
}
fn parameters(minimum: u32, mut read: impl FnMut() -> Option<u32>) -> Option<[u32; 3]> {
    let ctb = u32::wrapping_add(minimum, read()?);
    let tb = minimum.wrapping_add(read()?);
    let band = minimum + read()?;
    Some([ctb, tb, band])
}
fn branch(read: impl FnOnce() -> Option<bool>) -> Option<u8> {
    match read()? { true => Some(1), false => Some(2) }
}
fn projection(minimum: (u32, u32), read: impl FnOnce() -> Option<u32>) -> Option<u32> {
    Some(minimum.1 + read()?)
}
fn pixel(dst: &[u8], col: usize, effects: &Cell<u8>) -> Result<u32, ()> {
    let channel = u32::from(*(<[_]>::get(&*dst, col).ok_or(())?));
    effects.set(effects.get() + 1);
    Ok(channel)
}
fn conditional(read: impl FnOnce() -> Option<bool>) -> Option<u8> {
    if read()? { Some(3) } else { Some(4) }
}
fn main() {
    let mut bounds = Bounds { prev_end: 4, end: 1 };
    assert_eq!(bounds.extend(5), Some(()));
    assert_eq!(bounds.end, 9);
    assert_eq!(bounds.extend(u32::MAX), None);
    assert_eq!(bounds.end, 9);
    let trace = Cell::new(0);
    let mut row = [3];
    assert_eq!(write(&mut row, &trace, false), None);
    assert_eq!((row, trace.get()), ([3], 1));
    trace.set(0);
    assert_eq!(write(&mut row, &trace, true), Some(()));
    assert_eq!((row, trace.get()), ([7], 12));
    let mut words = [1u32, 2, 3].into_iter();
    assert_eq!(parameters(4, || words.next()), Some([5, 6, 7]));
    assert_eq!(parameters(4, || None), None);
    assert_eq!(branch(|| Some(false)), Some(2));
    assert_eq!(projection((9, 7), || Some(3)), Some(10));
    let effects = Cell::new(0);
    assert_eq!(pixel(&[17], 0, &effects), Ok(17));
    assert_eq!(pixel(&[], 0, &effects), Err(()));
    assert_eq!(effects.get(), 1);
    assert_eq!(conditional(|| None), None);
}
