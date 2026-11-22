use std::convert::TryInto;

pub fn fixed_index(value: usize) -> Result<u64, core::convert::Infallible> {
    Ok(value as u64)
}

pub fn index(value: u64) -> Result<usize, core::convert::Infallible> {
    Ok(value.try_into().unwrap_or(usize::MAX))
}
