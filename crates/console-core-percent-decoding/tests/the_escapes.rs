//! What an address spells, asked the way its three readers ask it.

use console_core_percent_decoding::{decoded, escaped_byte};

#[test]
fn an_escape_is_the_byte_its_two_digits_spell() {
  assert_eq!(decoded(b"a%20b"), Ok(b"a b".to_vec()));
  assert_eq!(decoded(b"%41%6a%6A"), Ok(b"Ajj".to_vec()));
  assert_eq!(decoded(b"%00%ff"), Ok(vec![0x00, 0xff]));
}

#[test]
fn a_letter_in_two_bytes_comes_back_as_both() {
  assert_eq!(
    decoded(b"Beyonc%C3%A9"),
    Ok("Beyonc\u{e9}".as_bytes().to_vec())
  );
}

#[test]
fn a_percent_that_escapes_nothing_is_a_percent() {
  assert_eq!(decoded(b"100%"), Ok(b"100%".to_vec()));
  assert_eq!(decoded(b"100%4"), Ok(b"100%4".to_vec()));
  assert_eq!(decoded(b"%zz"), Ok(b"%zz".to_vec()));
  assert_eq!(decoded(b"%%41"), Ok(b"%A".to_vec()));
}

#[test]
fn a_sign_is_not_a_digit() {
  assert_eq!(decoded(b"%+5"), Ok(b"%+5".to_vec()));
  assert_eq!(decoded(b"%-0"), Ok(b"%-0".to_vec()));
  assert_eq!(escaped_byte(b"+5"), Ok(None));
}

#[test]
fn an_escape_is_read_once() {
  assert_eq!(decoded(b"%2541"), Ok(b"%41".to_vec()));
}

#[test]
fn nothing_is_nothing() {
  assert_eq!(decoded(b""), Ok(Vec::new()));
  assert_eq!(escaped_byte(b""), Ok(None));
  assert_eq!(escaped_byte(b"4"), Ok(None));
}

#[test]
fn two_digits_are_asked_at_the_front_of_what_is_left() {
  assert_eq!(escaped_byte(b"41 and the rest"), Ok(Some(b'A')));
  assert_eq!(escaped_byte(b"g1"), Ok(None));
}
