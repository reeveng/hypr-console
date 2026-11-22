// run-rustfix
// edition:2024

fn marker(bytes: &[u8], enabled: bool) -> bool {
    enabled && bytes.len() > 1 && bytes[0] == b'!' && bytes[1].is_ascii_alphabetic()
}

fn skip_two(bytes: &[u8]) -> bool {
    bytes.len() < 3 || bytes[1] == b'!'
}

fn digits(bytes: &[u8]) -> usize {
    let mut position = 0;
    while position < bytes.len() && bytes[position].is_ascii_digit() {
        position += 1;
    }
    position
}

fn main() {
    assert!(!marker(b"!", true));
    assert!(!marker(b"!2", true));
    assert!(!marker(b"!a", false));
    assert!(marker(b"!a", true));
    assert!(marker(b"!ab", true));
    assert!(skip_two(b"ab"));
    assert!(!skip_two(b"abc"));
    assert!(skip_two(b"a!c"));
    assert_eq!(digits(b"123!"), 3);
    assert_eq!(digits(b""), 0);
}
