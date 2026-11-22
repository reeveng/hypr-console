// run-rustfix
// edition:2024
// aux-build:console_core_number_conversion.rs

extern crate console_core_number_conversion;

fn convert(input: &std::cell::RefCell<f64>, signed: i32, unsigned: u64, float: f32) {
    let mut int = float as u8;
    int += 1;
    assert_eq!(int, 1);

    let (narrow, free) = (input.borrow().clone() as f32, input.try_borrow_mut().is_ok());
    assert_eq!(narrow.to_bits(), (-0.0f32).to_bits());
    assert!(!free);

    let negative = (signed as f32).is_sign_negative();
    assert!(negative);
    let sum = unsigned as f32 + 1.0;
    assert_eq!(sum, 5.0);
}

fn main() {
    convert(&std::cell::RefCell::new(-0.0), -1, 4, f32::NAN);
}
