// run-rustfix
// edition:2024

#[derive(Clone, Copy)]
enum Scaling {
    Apply,
    Skip,
}

fn scale(value: i32, apply_scale: bool) -> i32 {
    match ::core::primitive::bool::then_some(apply_scale, ()) {
        ::core::option::Option::Some(()) => value * 2,
        ::core::option::Option::None => value,
    }
}

fn forward(value: i32, apply_scale: bool) -> i32 {
    scale(value, apply_scale)
}

fn main() {
    assert_eq!(forward(4, true), 8);
    let depends_on_input = std::env::args().len() > 1;
    let _ = forward(4, depends_on_input);
    assert_eq!(scale(4, false), 4);
}
