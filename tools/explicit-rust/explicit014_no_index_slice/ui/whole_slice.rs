// run-rustfix
// edition:2024

fn from_bytes() -> &'static [u8] {
    &b"bytes"[..]
}

fn reborrow(slice: &mut [u8]) -> &mut [u8] {
    &mut slice[..]
}

fn as_slice(bytes: &mut Vec<u8>) -> &mut [u8] {
    &mut bytes[..]
}

fn as_text(text: &String) -> &str {
    &text[..]
}

fn main() {
    assert_eq!(from_bytes(), b"bytes");
    let mut bytes = vec![1, 2];
    assert_eq!(as_slice(&mut bytes), &[1, 2]);
    reborrow(&mut bytes).fill(3);
    assert_eq!(as_slice(&mut bytes), &[3, 3]);
    assert_eq!(as_text(&String::from("bytes")), "bytes");
}
