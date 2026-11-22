pub fn wrapping_add_index(amount: usize, target: &mut usize) -> Result<(), core::convert::Infallible> {
    *target = target.wrapping_add(amount);
    Ok(())
}
