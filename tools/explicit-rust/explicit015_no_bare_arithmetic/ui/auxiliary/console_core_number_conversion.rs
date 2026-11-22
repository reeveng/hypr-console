pub use integer_bits::wrapped;

mod integer_bits {
    pub fn wrapped<F, T: From<F>>(value: F) -> Result<T, core::convert::Infallible> {
        Ok(T::from(value))
    }
}
