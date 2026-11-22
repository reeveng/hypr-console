// run-rustfix
// edition:2024
// compile-flags: --test

#[derive(Debug, PartialEq)]
enum ReadError { Truncated }

fn read(bytes: &[u8]) -> Result<u8, ReadError> {
    bytes.first().copied().ok_or(ReadError::Truncated)
}

fn glyph(id: u8) -> Option<&'static str> {
    match id { 0 => Some(".notdef"), _ => None }
}

#[test]
fn checks_a_font_reader() {
    assert_eq!(read(&[0xAA]).unwrap(), 0xAA);
    assert_eq!(0, read(&[0]).unwrap());
    assert_eq!(glyph(0).unwrap(), ".notdef");
    let expected = -3i32;
    assert_eq!(Some(-3i32).unwrap(), expected);
}

#[test]
fn consumes_a_reader_once() {
    let mut bytes = [12u8, 35].into_iter();
    assert_eq!(bytes.next().unwrap(), 12);
    assert_eq!(35, bytes.next().unwrap());
    assert!(bytes.next().is_none());
}

#[derive(Debug)]
struct ParseError;
fn read_int(value: i32) -> Result<i32, ParseError> { Ok(value) }

#[test]
fn postscript_operands() {
    assert_eq!(read_int(20).unwrap(), 20);
    assert_eq!(read_int(-102).unwrap().abs(), 102);
    assert_eq!(4, read_int(4).unwrap());
}

#[derive(Debug)]
enum Impossible {}
fn fixed_value() -> Result<u8, Impossible> { Ok(2) }

#[test]
fn infallible_reads_have_individual_edits() -> Result<(), Impossible> {
    let first = fixed_value().unwrap();
    assert_eq!(fixed_value().unwrap(), first);
    Ok(())
}
