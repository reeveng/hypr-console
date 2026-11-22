// run-rustfix
// edition:2024
// aux-build:console_core_number_conversion.rs
extern crate console_core_number_conversion;

fn read(bytes: &[u8], consumed: &[u8], count: &mut u8) -> Option<u8> {
    let row = { *count += 1; consumed.len() };
    bytes.get(row).copied()
}

fn last_two(bytes: &[u8]) -> Option<&[u8]> {
    let start = bytes.len().saturating_sub(2);
    bytes.get(start..)
}

fn main() {
    let mut reads = 0;
    assert_eq!(read(&[11, 27, 45], &[0], &mut reads), Some(27));
    assert_eq!(reads, 1);
    assert_eq!(read(&[], &[], &mut reads), None);
    assert_eq!(last_two(&[1, 2, 3]), Some(&[2, 3][..]));
    assert_eq!(last_two(&[]), Some(&[][..]));
}
