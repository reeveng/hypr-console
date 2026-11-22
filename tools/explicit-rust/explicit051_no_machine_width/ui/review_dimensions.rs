// edition:2024
// aux-build:console_core_number_conversion.rs
extern crate console_core_number_conversion;

#[allow(explicit051_no_machine_width)]
type Position = usize;

fn checked_on_each_target() -> Position {
    usize::wrapping_mul(512, 128)
}
fn main() {
    let _unknown = usize::wrapping_mul(64, 64);
    let _generic = std::hint::black_box(usize::wrapping_mul(64, 64));
    let _receiver = usize::wrapping_mul(16, 16).rotate_left(5);
    std::hint::black_box(checked_on_each_target());
}

fn second_is_too_late() {
    for row in 0..8 {
        let _pair = (usize::wrapping_mul(row, 8), Position::wrapping_mul(row, 8));
    }
}

fn closure_checks_its_arguments_later() {
    for row in 0..8 {
        let _pair = (|| Position::wrapping_mul(row, 8), usize::wrapping_mul(row, 8));
    }
}

fn conditional_lane() {
    for row in 0..8 {
        let _lanes = [
            #[cfg(not(feature = "scalar"))]
            Position::wrapping_mul(row, 8),
            usize::wrapping_mul(row, 8),
        ];
    }
}

fn keep_macro_spelling() -> (Position, &'static str) {
    macro_rules! with_source { ($e:expr) => { ($e, stringify!($e)) } }
    with_source!(usize::wrapping_mul(64, 64))
}

fn nested(value: Position) -> Position {
    usize::wrapping_mul(64, 64).wrapping_add(value)
}

fn nested_qualified() -> Position {
    usize::wrapping_sub(usize::wrapping_add(64, 7), 3)
}

fn traced(text: &str, value: Position) -> Position {
    println!("{text}");
    value
}

fn stringified_later_lane() {
    macro_rules! trace_input { ($e:expr) => { traced(stringify!($e), $e) } }
    for row in 0..8 {
        let _lanes = (
            Position::wrapping_mul(row, 8),
            trace_input!(usize::wrapping_mul(row, 8)),
        );
    }
}
