pub use assignments::{wrapping_add_index, wrapping_sub_offset};

pub fn index(value: u8) -> Result<usize, core::convert::Infallible> {
    Ok(usize::from(value))
}

mod assignments {
    pub fn wrapping_add_index(amount: usize, target: &mut usize) -> Result<(), core::convert::Infallible> {
        *target = target.wrapping_add(amount);
        Ok(())
    }

    pub fn wrapping_sub_offset(amount: isize, target: &mut isize) -> Result<(), core::convert::Infallible> {
        *target = target.wrapping_sub(amount);
        Ok(())
    }
}
