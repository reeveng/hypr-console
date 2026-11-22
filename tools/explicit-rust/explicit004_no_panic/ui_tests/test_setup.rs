// run-rustfix
// edition:2024
// compile-flags: --test

#[derive(Debug)]
enum ReadError { Truncated }

fn read(bytes: &[u8]) -> Result<u8, ReadError> {
    bytes.first().copied().ok_or(ReadError::Truncated)
}

#[test]
fn reads_two_headers() {
    let first = read(&[7]).unwrap();
    let second = read(&[3]).unwrap();
    assert_eq!(first + second, 10);
}

#[test]
fn already_fallible() -> Result<(), ReadError> {
    let header = read(&[4]).unwrap();
    assert_eq!(header, 4);
    Ok(())
}

#[test]
fn formatted_fixture() {
    #[rustfmt::skip]
        let bytes = [
            7, 3,
            2, 1,
        ];
    let header = read(&bytes).unwrap();
    assert_eq!(header, 7, "fixture #1");
}
