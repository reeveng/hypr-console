// run-rustfix
// edition:2024
// only-x86_64-unknown-linux-gnu


#[derive(Clone, Copy)]
enum Order {
    Normal,
    Reversed,
}

fn direction(flip: bool) -> i32 {
    match flip {
        true => -1,
        false => 1,
    }
}

macro_rules! by_literal {
    ($flip:expr) => {
        direction($flip)
    };
}

fn with_specialized_order<const FLIPPED: bool>() -> i32 {
    direction(FLIPPED)
}

fn main() {
    let _ = by_literal!(true);
    let _ = by_literal!(false);
    let _ = with_specialized_order::<false>();
}
