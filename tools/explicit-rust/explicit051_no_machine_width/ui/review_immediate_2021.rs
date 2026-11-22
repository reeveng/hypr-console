// edition:2021
use core::convert::Infallible;

#[allow(explicit051_no_machine_width)]
type Position = usize;

fn consume() -> Position {
    let Ok(position) = Result::<Position, Infallible>::Ok(7);
    position
}

fn main() {}
