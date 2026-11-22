// run-rustfix
// edition:2024

use core::convert::Infallible;

fn projected(answer: Result<usize, Infallible>) -> usize {
    answer.unwrap_or_else(|impossible| match impossible {})
}

fn byte(input: i32) -> Option<u8> { u8::try_from(input).ok() }
fn can_encode(input: i32) -> bool { u8::try_from(input).is_ok() }
fn utf8(bytes: &[u8]) -> bool {
    let read = core::str::from_utf8(bytes);
    let rejected = read.is_err();
    rejected && read.is_err()
}

fn main() {
    assert_eq!(projected(Ok(7)), 7);
    assert_eq!(byte(255), Some(255));
    assert_eq!(byte(-1), None);
    assert!(can_encode(0) && !can_encode(256));
    assert!(!utf8(b"hello") && utf8(&[0xff]));
}
