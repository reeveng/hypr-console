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

macro_rules! used_twice {
    ($flip:expr) => {{
        let _ = !$flip;
        direction($flip)
    }};
}

fn main() {
    let _ = used_twice!(true);
}
