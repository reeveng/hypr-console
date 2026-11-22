// run-rustfix
// edition:2024

use std::convert::Infallible;

fn extract(value: Result<String, Infallible>) -> usize {
    value.unwrap().len()
}

#[derive(Debug)]
enum Never {}

fn take(value: Result<&mut u8, Never>) {
    *value.unwrap() += 1;
}

fn extract_with_expect(value: Result<u8, Infallible>) -> u8 {
    value.expect("successful value")
}

fn main() {
    assert_eq!(extract(Ok(String::from("rust"))), 4);
    let mut value = 8;
    take(Ok(&mut value));
    assert_eq!(value, 9);
    assert_eq!(extract_with_expect(Ok(8)), 8);
}
