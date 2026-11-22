// edition:2024

pub fn unchecked_width(dst: &mut [u8], start: usize) -> Result<(), ()> {
    (*<[_]>::get_mut(&mut *dst, start..start + 2).ok_or(())?)
        .copy_from_slice(&12_u16.to_le_bytes());
    Ok(())
}

pub fn mismatched_width(dst: &mut [u8], start: usize) -> Result<(), ()> {
    (*<[_]>::get_mut(&mut *dst, start..start.wrapping_add(1)).ok_or(())?)
        .copy_from_slice(&12_u16.to_le_bytes());
    Ok(())
}

fn main() {}
