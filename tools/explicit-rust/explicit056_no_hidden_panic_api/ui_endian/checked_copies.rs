// run-rustfix
// edition:2024

#[derive(Debug, PartialEq, Eq)]
enum CopyError {
    Bounds,
}

fn write_le(
    dst: &mut [u8],
    start: usize,
    counter: &mut u32,
) -> Result<(), CopyError> {
    (*<[u8]>::get_mut(&mut *dst, start..(std::num::Wrapping(start) + std::num::Wrapping(2)).0)
        .ok_or(CopyError::Bounds)?)
        .copy_from_slice(&{
            *counter += 1;
            0x1234_u16
        }.to_le_bytes());
    Ok(())
}

fn write_be(dst: &mut [u8], start: usize, value: u32) -> Result<(), CopyError> {
    (*{
        let checked_value = <[_]>::get_mut(&mut *dst, start..start.wrapping_add(4))
            .ok_or(CopyError::Bounds)?;
        checked_value
    })
    .copy_from_slice(&value.to_be_bytes());
    Ok(())
}

fn main() {
    let mut dst = [0; 6];
    let mut count = 0;
    assert_eq!(write_le(&mut dst, 0, &mut count), Ok(()));
    assert_eq!(write_be(&mut dst, 2, 0x1234_5678), Ok(()));
    assert_eq!(dst, [0x34, 0x12, 0x12, 0x34, 0x56, 0x78]);
    assert_eq!(write_le(&mut dst, 5, &mut count), Err(CopyError::Bounds));
    assert_eq!(write_le(&mut dst, 7, &mut count), Err(CopyError::Bounds));
    assert_eq!(write_le(&mut dst, usize::MAX, &mut count), Err(CopyError::Bounds));
    assert_eq!(dst, [0x34, 0x12, 0x12, 0x34, 0x56, 0x78]);
    assert_eq!(count, 1);
}
