// run-rustfix
// edition:2024
// aux-build:console_core_number_conversion.rs

extern crate console_core_number_conversion;

#[derive(Debug, PartialEq)]
struct Bounds;

fn row(data: &[u8], low: i64, high: u64) -> Result<&[u8], Bounds> {
    let start = usize::try_from(low).map_err(|_| Bounds)?;
    let end = usize::try_from(high).map_err(|_| Bounds)?;
    data.get(start..end).ok_or(Bounds)
}

fn signed(value: i128) -> Result<bool, core::num::TryFromIntError> {
    let offset = isize::try_from(value)?;
    Ok(offset < 0)
}

fn main() {
    assert_eq!(row(&[4, 8], 1, 2), Ok(&[8][..]));
    assert_eq!(row(&[4, 8], -1, 1), Err(Bounds));
    assert_eq!(signed(-2), Ok(true));
    assert!(signed(i128::MAX).is_err());
}
