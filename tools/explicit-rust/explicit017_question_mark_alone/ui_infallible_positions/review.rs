// edition:2024
use std::convert::Infallible;

fn convert<E>() -> Result<usize, E> { Ok(0) }
fn take(_: usize) {}

fn generic_error() -> Result<(), Infallible> {
    take(convert::<Infallible>()?);
    Ok(())
}

fn may_fail() -> Result<usize, &'static str> { Err("missing") }

fn fallible() -> Result<(), &'static str> {
    take(may_fail()?);
    Ok(())
}

fn main() {}
