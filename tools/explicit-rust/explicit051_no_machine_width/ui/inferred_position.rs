// edition:2024
use std::convert::Infallible;

fn carry<T>(value: T) -> Result<T, Infallible> { Ok(value) }

fn main() {
    let Ok(value) = carry(1);
    println!("{}", usize::wrapping_mul(value, 2));
}
