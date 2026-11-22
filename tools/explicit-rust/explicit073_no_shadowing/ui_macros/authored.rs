// run-rustfix
// edition:2024
// aux-build: body_wrapper.rs

extern crate body_wrapper;

#[body_wrapper::keep_body]
fn wrapped(input: u8) -> u8 {
    let value = input + 1;
    let value = value + 1;
    value
}

fn main() {
    assert_eq!(wrapped(4), 6);
}
