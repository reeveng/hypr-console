// edition:2024
fn combine(left: u8, right: u8) -> u8 { left + right }
fn mutable_read(mut before: u8) -> Option<(u8, u8)> {
    let result = combine(before, { before = 7; Some(4) }?);
    Some((result, before))
}
struct Field { before: u8 }
fn mutable_field(data: &mut Field) -> Option<u8> {
    let value = combine(data.before, { data.before = 7; Some(4) }?);
    Some(value)
}
fn repeated(mut read: impl FnMut() -> Option<bool>) -> Option<u8> {
    let mut count = 0;
    while read()? { count += 1; }
    Some(count)
}
fn borrowed_scrutinee(value: Option<u8>) -> Option<u8> {
    match value? {
        ref mut byte => { *byte += 1; Some(*byte) }
    }
}
fn compound(data: &mut Field, read: impl FnOnce() -> Option<u8>) -> Option<()> {
    data.before += read()?;
    Some(())
}
fn take(data: Field) -> Option<u8> { Some(data.before) }
fn moved_aggregate(data: Field) -> Option<u8> {
    Some(combine(data.before, take(data)?))
}
fn guarded_pixel(src: &[u8], index: &std::cell::RefCell<usize>) -> Option<u32> {
    let value = u32::from(*(src.get(*index.borrow())?));
    Some(value)
}

fn observed_borrow(values: &std::cell::RefCell<Vec<u8>>) -> Option<u8> {
    let value = values.borrow().first().copied()?.saturating_add(
        u8::from(values.try_borrow_mut().is_ok()),
    );
    Some(value)
}
fn inspect(value: &std::cell::Cell<u8>) -> u8 { value.get() }
fn borrowed_temporary() -> Option<u8> {
    Some(inspect(Some(&std::cell::Cell::new(3))?))
}
fn borrowed_pattern() -> Option<u8> {
    Some(inspect((match std::cell::Cell::new(4) { ref cell => Some(cell) })?))
}
fn main() {
    assert_eq!(mutable_read(2), Some((6, 7)));
    let mut data = Field { before: 2 };
    assert_eq!(mutable_field(&mut data), Some(6));
    assert_eq!(data.before, 7);
    let mut conditions = [true, false].into_iter();
    assert_eq!(repeated(|| conditions.next()), Some(1));
    assert_eq!(borrowed_scrutinee(Some(3)), Some(4));
    assert_eq!(moved_aggregate(data), Some(14));
    assert_eq!(guarded_pixel(&[6], &std::cell::RefCell::new(0)), Some(6));
    assert_eq!(borrowed_temporary(), Some(3));
    assert_eq!(borrowed_pattern(), Some(4));
}
