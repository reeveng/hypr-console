#![allow(explicit051_no_machine_width, reason = "this auxiliary crate models the one boundary that converts held positions and ranges to standard-library indices")]

pub fn fitted<F: Into<i128>, T: core::convert::TryFrom<i128> + Default>(value: F) -> Result<T, core::convert::Infallible> {
    Ok(T::try_from(value.into()).unwrap_or_default())
}

pub fn index<F: Into<i128>>(value: F) -> Result<usize, core::convert::Infallible> {
    fitted(value)
}

pub fn indices<F: Into<i128>>(value: core::ops::Range<F>) -> Result<core::ops::Range<usize>, core::convert::Infallible> {
    let Ok(start) = index(value.start);
    let Ok(end) = index(value.end);
    Ok(start..end)
}

pub fn indices_from(value: core::ops::RangeFrom<u32>) -> Result<core::ops::RangeFrom<usize>, core::convert::Infallible> {
    Ok(value.start as usize..)
}

pub fn indices_to(value: core::ops::RangeTo<u32>) -> Result<core::ops::RangeTo<usize>, core::convert::Infallible> {
    Ok(..value.end as usize)
}

pub fn indices_inclusive<F: Into<i128>>(value: core::ops::RangeInclusive<F>) -> Result<core::ops::RangeInclusive<usize>, core::convert::Infallible> {
    let (start, end) = value.into_inner();
    let Ok(start) = index(start);
    let Ok(end) = index(end);
    Ok(start..=end)
}

pub fn indices_to_inclusive(value: core::ops::RangeToInclusive<u32>) -> Result<core::ops::RangeToInclusive<usize>, core::convert::Infallible> {
    Ok(..=value.end as usize)
}

pub fn wrapped_index<F: Into<i128>>(value: F) -> Result<usize, core::convert::Infallible> {
    Ok(value.into() as usize)
}

pub fn wrapped_offset<F: Into<i128>>(value: F) -> Result<isize, core::convert::Infallible> {
    Ok(value.into() as isize)
}

pub fn toward_zero_usize(value: f64) -> Result<usize, core::convert::Infallible> {
    Ok(value as usize)
}

pub fn wrapping_add_index(amount: usize, target: &mut usize) -> Result<(), core::convert::Infallible> {
    *target = target.wrapping_add(amount);
    Ok(())
}

pub fn wrapping_sub_offset(amount: isize, target: &mut isize) -> Result<(), core::convert::Infallible> {
    *target = target.wrapping_sub(amount);
    Ok(())
}

pub fn wrapping_mul_index(amount: usize, target: &mut usize) -> Result<(), core::convert::Infallible> {
    *target = target.wrapping_mul(amount);
    Ok(())
}

pub fn checked_index<F>(value: F) -> Result<usize, <usize as core::convert::TryFrom<F>>::Error>
where
    usize: core::convert::TryFrom<F>,
{
    <usize as core::convert::TryFrom<F>>::try_from(value)
}

pub fn fixed_index(value: usize) -> Result<u64, core::convert::Infallible> {
    Ok(value as u64)
}

pub fn fixed_offset(value: isize) -> Result<i64, core::convert::Infallible> {
    Ok(value as i64)
}

pub fn checked_offset<F>(value: F) -> Result<isize, <isize as core::convert::TryFrom<F>>::Error>
where
    isize: core::convert::TryFrom<F>,
{
    <isize as core::convert::TryFrom<F>>::try_from(value)
}
