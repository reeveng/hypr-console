// run-rustfix
// edition:2024
// aux-build:console_core_number_conversion.rs

extern crate console_core_number_conversion;

fn signed_low_bits(value: i32) -> u8 {
    value as u8
}

fn unsigned_low_bits(value: u64) -> i16 {
    value as i16
}

fn keep_input_alive(input: &std::cell::RefCell<i32>) -> (u8, bool) {
    (input.borrow().clone() as u8, input.try_borrow_mut().is_err())
}

fn main() {
    assert_eq!(signed_low_bits(-257), 255);
    assert_eq!(unsigned_low_bits(65535), -1);
    assert_eq!(keep_input_alive(&std::cell::RefCell::new(-1)), (255, true));
}
