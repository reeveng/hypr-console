// edition:2024
use core::convert::Infallible;

#[allow(explicit051_no_machine_width)]
type Position = usize;

enum Slot { Ok(Position) }

fn opaque() -> Position {
    let Slot::Ok(value) = Slot::Ok(2);
    value
}

fn multiple() -> Result<Position, Infallible> {
    let Ok(value) = Result::<Position, Infallible>::Ok(3);
    let _next = value + 1;
    Ok(value)
}

macro_rules! spelling { ($e:expr) => { (stringify!($e), $e) } }

fn main() {
    let _ = spelling!({
        let Ok(value) = Result::<Position, Infallible>::Ok(3);
        value
    });
}
