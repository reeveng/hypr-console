// run-rustfix
// edition:2024

fn row(offset: Option<u32>, width: u32) -> Option<u32> {
    let end = offset?.checked_add(width)?.checked_mul(2)?;
    Some(end)
}

fn read_pair(mut read: impl FnMut() -> Option<u16>) -> Option<[u16; 2]> {
    let pair = [read()?, 3];
    Some(pair)
}

fn flag(mut read: impl FnMut() -> Option<bool>, keep: bool) -> Option<bool> {
    let answer = !read()? && keep;
    Some(answer)
}

fn main() {
    assert_eq!(row(Some(2), 4), Some(12));
    assert_eq!(row(None, 4), None);
    assert_eq!(row(Some(u32::MAX), 4), None);
    assert_eq!(read_pair(|| Some(1)), Some([1, 3]));
    assert_eq!(flag(|| Some(false), true), Some(true));
}
