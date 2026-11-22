use console_core_number_conversion::FromBytes;

#[test]
fn lexical_digits_are_checked_against_the_destination_width() {
  assert_eq!(u32::from_bytes_dec(b"4294967295"), Some(u32::MAX));
  assert_eq!(u32::from_bytes_dec(b"4294967296"), None);
  assert_eq!(u32::from_bytes_hex(b"fFfFfFfF"), Some(u32::MAX));
  assert_eq!(u32::from_bytes_hex(b"100000000"), None);
  assert_eq!(i64::from_bytes_dec(b"9223372036854775807"), Some(i64::MAX));
  assert_eq!(i64::from_bytes_dec(b"9223372036854775808"), None);
  assert_eq!(i64::from_bytes_hex(b"10000000000000000"), None);
}

#[test]
fn signs_prefixes_and_whitespace_belong_to_the_caller() {
  for bytes in [
    b"".as_slice(),
    b"-1",
    b"+1",
    b" 1",
    b"1 ",
    b"1.0",
    b"0x10",
    b"A",
  ] {
    assert_eq!(u32::from_bytes_dec(bytes), None);
  }

  assert_eq!(u32::from_bytes_dec(b"0000000010"), Some(10));
  assert_eq!(u32::from_bytes_hex(b"0x10"), None);
}
